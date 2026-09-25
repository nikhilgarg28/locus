//! Readability cleanup after erasure, before printing. Only marker storage
//! is deleted: runtime values retain their bindings and destruction scopes.
//! Calls, operators, panics and control flow are never assumed effect free.

use std::collections::{HashMap, HashSet};

use crate::kernel::VarId;
use crate::typed::Passing;

use super::tree::{EBlock, EExpr, EFn, EPattern, EStmt, EType, Module};

pub(super) fn module(original: &Module) -> Module {
    let mut module = original.clone();
    for function in &mut module.fns {
        clean_function(function, original);
    }
    module
}

fn marker(ty: &EType) -> bool {
    matches!(ty, EType::Proved | EType::Ghost)
}

#[derive(Default)]
struct Uses {
    read: HashSet<VarId>,
    written: HashSet<VarId>,
    markers: HashSet<VarId>,
}

fn clean_function(function: &mut EFn, module: &Module) {
    loop {
        let uses = uses(function, module);
        let mut changed = false;
        clean_block(&mut function.body, &uses, &mut changed);
        if !changed {
            break;
        }
    }
    let uses = uses(function, module);
    let mut names = HashMap::new();
    for (index, (id, name, _)) in function.params.iter_mut().enumerate() {
        if !(uses.read.contains(id) || function.receiver && index == 0) {
            names.insert(*id, unused_name(name));
        }
    }
    collect_unused(&mut function.body, &uses, &mut names);
    avoid_collisions(function, &mut names);
    // A name is changed by identity, not text: shadows remain distinct.
    for (index, (id, name, _)) in function.params.iter_mut().enumerate() {
        if let Some(new) = names.get(id) {
            *name = new.clone();
        }
        if function.passing.get(index) == Some(&Passing::MutValue) && !uses.written.contains(id) {
            function.passing[index] = Passing::Value;
        }
    }
    rename_block(&mut function.body, &uses, &names);
}

/// Prefixing an unused `x` must not shadow an existing, used `_x`.
fn avoid_collisions(function: &EFn, renames: &mut HashMap<VarId, String>) {
    #[derive(Default)]
    struct Names {
        bindings: Vec<(VarId, String)>,
    }
    impl Names {
        fn pattern(&mut self, pattern: &EPattern) {
            match pattern {
                EPattern::Bind { id, name, .. } => self.bindings.push((*id, name.clone())),
                EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => {
                    for part in parts {
                        self.pattern(part);
                    }
                }
                _ => {}
            }
        }
    }
    impl Visit for Names {
        fn stmt(&mut self, stmt: &EStmt) {
            if let EStmt::Let { pattern, .. } = stmt {
                self.pattern(pattern);
            }
            walk_stmt(self, stmt);
        }
        fn expr(&mut self, expr: &EExpr) {
            match expr {
                EExpr::For { index, .. } => self.bindings.push((index.0, index.1.clone())),
                EExpr::Match { arms, .. } => {
                    for arm in arms {
                        self.bindings.extend(arm.payload.iter().cloned());
                    }
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut names = Names {
        bindings: function
            .params
            .iter()
            .map(|(id, name, _)| (*id, name.clone()))
            .collect(),
    };
    names.block(&function.body);
    let mut occupied: HashSet<String> = names
        .bindings
        .iter()
        .map(|(_, name)| name.clone())
        .collect();
    for (id, old) in names.bindings {
        let Some(new) = renames.get_mut(&id) else {
            continue;
        };
        if *new == "_" || *new == old {
            continue;
        }
        let base = new.clone();
        let mut suffix = 0;
        while occupied.contains(new) {
            suffix += 1;
            *new = format!("{base}_{suffix}");
        }
        occupied.insert(new.clone());
    }
}

fn unused_name(name: &str) -> String {
    if name.starts_with('_') {
        name.into()
    } else {
        format!("_{name}")
    }
}

fn uses(function: &EFn, module: &Module) -> Uses {
    let mut uses = Uses::default();
    for (index, (id, _, ty)) in function.params.iter().enumerate() {
        // A reference points to caller storage, never a removable local.
        if marker(ty) && !function.passing_of(index).is_reference() {
            uses.markers.insert(*id);
        }
    }
    fn pattern(pat: &EPattern, uses: &mut Uses) {
        match pat {
            EPattern::Bind { id, ty, .. } if marker(ty) => {
                uses.markers.insert(*id);
            }
            EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => {
                for part in parts {
                    pattern(part, uses);
                }
            }
            _ => {}
        }
    }
    struct Scan<'a> {
        uses: &'a mut Uses,
        module: &'a Module,
        references: HashSet<VarId>,
        reachable: bool,
    }
    impl Visit for Scan<'_> {
        fn block(&mut self, block: &EBlock) {
            let before = self.reachable;
            for stmt in &block.stmts {
                self.stmt(stmt);
                let value = match stmt {
                    EStmt::Let { value, .. } | EStmt::Assign { value, .. } | EStmt::Expr(value) => {
                        value
                    }
                };
                if super::rust::diverges(value) {
                    self.reachable = false;
                }
            }
            if let Some(tail) = &block.tail {
                self.expr(tail);
            }
            self.reachable = before;
        }
        fn stmt(&mut self, statement: &EStmt) {
            match statement {
                EStmt::Let { pattern: pat, .. } => pattern(pat, self.uses),
                EStmt::Assign { place, .. } => {
                    self.uses.written.insert(place.id);
                    if !place.path.is_empty() || self.references.contains(&place.id) {
                        self.uses.read.insert(place.id);
                    }
                }
                _ => {}
            }
            walk_stmt(self, statement);
        }
        fn expr(&mut self, expression: &EExpr) {
            match expression {
                EExpr::Var { id, .. } if self.reachable => {
                    self.uses.read.insert(*id);
                }
                EExpr::Lend { place, mutable } => {
                    if self.reachable {
                        self.uses.read.insert(place.id);
                    }
                    if *mutable {
                        self.uses.written.insert(place.id);
                    }
                }
                EExpr::Match {
                    enum_name, arms, ..
                } => {
                    if let Some(item) = self
                        .module
                        .enums
                        .iter()
                        .find(|item| item.name == *enum_name)
                    {
                        for arm in arms {
                            if let Some(variant) = item
                                .variants
                                .iter()
                                .find(|variant| variant.name == arm.variant_name)
                            {
                                for ((id, _), ty) in arm.payload.iter().zip(&variant.payload) {
                                    if marker(ty) {
                                        self.uses.markers.insert(*id);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            walk_expr(self, expression);
        }
    }
    Scan {
        uses: &mut uses,
        module,
        reachable: true,
        references: function
            .params
            .iter()
            .enumerate()
            .filter(|(index, _)| function.passing_of(*index).is_reference())
            .map(|(_, (id, _, _))| *id)
            .collect(),
    }
    .block(&function.body);
    uses
}

fn discardable(expression: &EExpr, uses: &Uses) -> bool {
    match expression {
        EExpr::Proved | EExpr::Ghost => true,
        EExpr::Var { id, .. } => uses.markers.contains(id),
        // A tuple is removable only when every component is itself an erased
        // value. In particular a runtime value's move/drop is not discarded.
        EExpr::Tuple(fields) => fields.iter().all(|field| discardable(field, uses)),
        _ => false,
    }
}

fn clean_pattern(pattern: &mut EPattern, uses: &Uses, changed: &mut bool) {
    match pattern {
        EPattern::Bind { id, ty, .. } if marker(ty) && !uses.read.contains(id) => {
            *pattern = EPattern::Wildcard;
            *changed = true;
        }
        EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => {
            for part in parts {
                clean_pattern(part, uses, changed);
            }
        }
        _ => {}
    }
}

fn all_wild(pattern: &EPattern) -> bool {
    match pattern {
        EPattern::Wildcard => true,
        EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => parts.iter().all(all_wild),
        _ => false,
    }
}

fn clean_block(block: &mut EBlock, uses: &Uses, changed: &mut bool) {
    let mut retained = Vec::new();
    for mut stmt in std::mem::take(&mut block.stmts) {
        let value = match &mut stmt {
            EStmt::Let { value, .. } | EStmt::Assign { value, .. } | EStmt::Expr(value) => value,
        };
        map_blocks(value, &mut |block| clean_block(block, uses, changed));
        if super::rust::diverges(value) {
            // No binding/store completes, and no marker tail can be reached.
            // Keep the diverging operation itself as the block's tail.
            block.tail = Some(Box::new(value.clone()));
            block.stmts = retained;
            *changed = true;
            return;
        }
        match &mut stmt {
            EStmt::Let { pattern, value } => {
                clean_pattern(pattern, uses, changed);
                if all_wild(pattern) && discardable(value, uses) {
                    *changed = true;
                    continue;
                }
            }
            EStmt::Assign { place, value }
                if place.path.is_empty()
                    && uses.markers.contains(&place.id)
                    && !uses.read.contains(&place.id) =>
            {
                *changed = true;
                if discardable(value, uses) {
                    continue;
                }
                stmt = EStmt::Let {
                    pattern: EPattern::Wildcard,
                    value: value.clone(),
                };
            }
            EStmt::Expr(value) if discardable(value, uses) => {
                *changed = true;
                continue;
            }
            _ => {}
        }
        retained.push(stmt);
    }
    block.stmts = retained;
    if let Some(tail) = &mut block.tail {
        map_blocks(tail, &mut |block| clean_block(block, uses, changed));
    }
}

fn collect_unused(block: &mut EBlock, uses: &Uses, names: &mut HashMap<VarId, String>) {
    fn pattern(pat: &EPattern, uses: &Uses, names: &mut HashMap<VarId, String>) {
        match pat {
            EPattern::Bind { id, name, .. } if !uses.read.contains(id) => {
                names.insert(*id, unused_name(name));
            }
            EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => {
                for part in parts {
                    pattern(part, uses, names);
                }
            }
            _ => {}
        }
    }
    for stmt in &block.stmts {
        if let EStmt::Let { pattern: pat, .. } = stmt {
            pattern(pat, uses, names);
        }
    }
    fn nested(expr: &mut EExpr, uses: &Uses, names: &mut HashMap<VarId, String>) {
        match expr {
            EExpr::Match { arms, .. } => {
                for arm in arms {
                    for (id, name) in &arm.payload {
                        if !uses.read.contains(id) {
                            names.insert(
                                *id,
                                if uses.markers.contains(id) {
                                    "_".into()
                                } else {
                                    unused_name(name)
                                },
                            );
                        }
                    }
                }
            }
            EExpr::For { index, .. } if !uses.read.contains(&index.0) => {
                names.insert(index.0, unused_name(&index.1));
            }
            _ => {}
        }
        map_children(expr, &mut |child| nested(child, uses, names));
        immediate_blocks(expr, &mut |block| collect_unused(block, uses, names));
    }
    for stmt in &mut block.stmts {
        match stmt {
            EStmt::Let { value, .. } | EStmt::Assign { value, .. } | EStmt::Expr(value) => {
                nested(value, uses, names)
            }
        }
    }
    if let Some(tail) = &mut block.tail {
        nested(tail, uses, names);
    }
}

fn rename_block(block: &mut EBlock, uses: &Uses, names: &HashMap<VarId, String>) {
    fn pattern(pat: &mut EPattern, uses: &Uses, names: &HashMap<VarId, String>) {
        match pat {
            EPattern::Bind {
                id, name, mutable, ..
            } => {
                if let Some(new) = names.get(id) {
                    *name = new.clone();
                }
                *mutable &= uses.written.contains(id);
            }
            EPattern::Tuple(parts) | EPattern::Struct { parts, .. } => {
                for part in parts {
                    pattern(part, uses, names);
                }
            }
            _ => {}
        }
    }
    fn expr(expression: &mut EExpr, uses: &Uses, names: &HashMap<VarId, String>) {
        match expression {
            EExpr::Var { id, name } => {
                if let Some(new) = names.get(id) {
                    *name = new.clone();
                }
            }
            EExpr::Lend { place, .. } => {
                if let Some(new) = names.get(&place.id) {
                    place.name = new.clone();
                }
            }
            EExpr::For { index, .. } => {
                if let Some(new) = names.get(&index.0) {
                    index.1 = new.clone();
                }
            }
            EExpr::Match { arms, .. } => {
                for arm in arms {
                    for (id, name) in &mut arm.payload {
                        if let Some(new) = names.get(id) {
                            *name = new.clone();
                        }
                    }
                }
            }
            _ => {}
        }
        map_children(expression, &mut |child| expr(child, uses, names));
        immediate_blocks(expression, &mut |block| rename_block(block, uses, names));
    }
    for stmt in &mut block.stmts {
        match stmt {
            EStmt::Let {
                pattern: pat,
                value,
            } => {
                pattern(pat, uses, names);
                expr(value, uses, names);
            }
            EStmt::Assign { place, value } => {
                if let Some(new) = names.get(&place.id) {
                    place.name = new.clone();
                }
                expr(value, uses, names);
            }
            EStmt::Expr(value) => expr(value, uses, names),
        }
    }
    if let Some(tail) = &mut block.tail {
        expr(tail, uses, names);
    }
}

/// Read-only visitor shared with the printer's targeted lint analysis.
pub(super) trait Visit: Sized {
    fn block(&mut self, block: &EBlock) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
        if let Some(tail) = &block.tail {
            self.expr(tail);
        }
    }
    fn stmt(&mut self, stmt: &EStmt) {
        walk_stmt(self, stmt);
    }
    fn expr(&mut self, expr: &EExpr) {
        walk_expr(self, expr);
    }
}

pub(super) fn walk_stmt(visitor: &mut impl Visit, stmt: &EStmt) {
    match stmt {
        EStmt::Let { value, .. } | EStmt::Assign { value, .. } | EStmt::Expr(value) => {
            visitor.expr(value)
        }
    }
}

pub(super) fn walk_expr(visitor: &mut impl Visit, expr: &EExpr) {
    match expr {
        EExpr::Buffer {
            arguments: parts, ..
        }
        | EExpr::Tuple(parts)
        | EExpr::Variant { payload: parts, .. }
        | EExpr::Operate {
            operands: parts, ..
        }
        | EExpr::NativeCall {
            arguments: parts, ..
        }
        | EExpr::Call {
            arguments: parts, ..
        } => {
            for part in parts {
                visitor.expr(part);
            }
        }
        EExpr::Struct { fields, .. } => {
            for (_, part) in fields {
                visitor.expr(part);
            }
        }
        EExpr::Method {
            receiver,
            arguments,
            ..
        } => {
            visitor.expr(receiver);
            for part in arguments {
                visitor.expr(part);
            }
        }
        EExpr::Compare { left, right, .. } => {
            visitor.expr(left);
            visitor.expr(right);
        }
        EExpr::BoxNew(value)
        | EExpr::BoxDeref(value)
        | EExpr::Shared { value, .. }
        | EExpr::Deref(value)
        | EExpr::Field { target: value, .. }
        | EExpr::Cast { expr: value, .. }
        | EExpr::Return(value)
        | EExpr::Break(Some(value))
        | EExpr::Assert {
            condition: value, ..
        } => visitor.expr(value),
        EExpr::If {
            condition,
            then_block,
            else_block,
        } => {
            visitor.expr(condition);
            visitor.block(then_block);
            visitor.block(else_block);
        }
        EExpr::Match {
            scrutinee, arms, ..
        } => {
            visitor.expr(scrutinee);
            for arm in arms {
                visitor.block(&arm.body);
            }
        }
        EExpr::Block(block) | EExpr::Loop { body: block, .. } => visitor.block(block),
        EExpr::While { condition, body } => {
            visitor.expr(condition);
            visitor.block(body);
        }
        EExpr::For { lo, hi, body, .. } => {
            visitor.expr(lo);
            visitor.expr(hi);
            visitor.block(body);
        }
        EExpr::Var { .. }
        | EExpr::Bool(_)
        | EExpr::Literal(..)
        | EExpr::Proved
        | EExpr::Ghost
        | EExpr::Lend { .. }
        | EExpr::Break(None)
        | EExpr::Continue
        | EExpr::Trap
        | EExpr::Panic { .. } => {}
    }
}

/// Visit direct expression children, leaving embedded blocks to the caller.
fn map_children(expr: &mut EExpr, visit: &mut impl FnMut(&mut EExpr)) {
    match expr {
        EExpr::Buffer {
            arguments: parts, ..
        }
        | EExpr::Tuple(parts)
        | EExpr::Variant { payload: parts, .. }
        | EExpr::Operate {
            operands: parts, ..
        }
        | EExpr::NativeCall {
            arguments: parts, ..
        }
        | EExpr::Call {
            arguments: parts, ..
        } => {
            for part in parts {
                visit(part);
            }
        }
        EExpr::Struct { fields, .. } => {
            for (_, part) in fields {
                visit(part);
            }
        }
        EExpr::Method {
            receiver,
            arguments,
            ..
        } => {
            visit(receiver);
            for part in arguments {
                visit(part);
            }
        }
        EExpr::Compare { left, right, .. } => {
            visit(left);
            visit(right);
        }
        EExpr::Field { target: value, .. }
        | EExpr::Cast { expr: value, .. }
        | EExpr::Return(value)
        | EExpr::Break(Some(value))
        | EExpr::Assert {
            condition: value, ..
        }
        | EExpr::If {
            condition: value, ..
        }
        | EExpr::While {
            condition: value, ..
        }
        | EExpr::Match {
            scrutinee: value, ..
        } => visit(value),
        EExpr::For { lo, hi, .. } => {
            visit(lo);
            visit(hi);
        }
        _ => {}
    }
}

fn immediate_blocks(expr: &mut EExpr, visit: &mut impl FnMut(&mut EBlock)) {
    match expr {
        EExpr::Block(block)
        | EExpr::Loop { body: block, .. }
        | EExpr::While { body: block, .. }
        | EExpr::For { body: block, .. } => visit(block),
        EExpr::If {
            then_block,
            else_block,
            ..
        } => {
            visit(then_block);
            visit(else_block);
        }
        EExpr::Match { arms, .. } => {
            for arm in arms {
                visit(&mut arm.body);
            }
        }
        _ => {}
    }
}

fn map_blocks(expr: &mut EExpr, visit: &mut impl FnMut(&mut EBlock)) {
    map_children(expr, &mut |child| map_blocks(child, visit));
    immediate_blocks(expr, visit);
}
