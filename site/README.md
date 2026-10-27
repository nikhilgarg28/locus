# Website

The website is built from the repository by `cargo xtask site`, and published to GitHub Pages on every push to `main`.

- `index.html` and `style.css` are the landing page, copied to the root of the output.
- Everything else is an [mdBook](https://rust-lang.github.io/mdBook/) under `docs/`, built from staged copies of the design documents. The staging step, in [xtask/src/site.rs](../xtask/src/site.rs), generates the table of contents, a page per litmus topic, and the RFC, issue, and error-code indexes. It also turns spec rule markers into anchors that list their examples and open issues, shows each issue's kind and status as badges, and points links to unpublished files at GitHub.
- `book.toml` configures mdBook, `book.css` styles the generated parts, and `intro.md` is the first page of the book.
- `{{commit}}`, `{{commit-url}}` and `{{date}}` in `index.html` and `intro.md` are filled in with the commit the site was built from.

The site is built only from sources that pass `cargo xtask check`. Links are relative, so the site works at any path, such as `/locus/`.

To preview it, run `cargo xtask site` and open `target/site/index.html`, or serve the directory with `python3 -m http.server -d target/site`.

To restyle the book beyond `book.css`, add a `theme/` directory as described in mdBook's documentation, and copy it into the staging directory in `site.rs`.
