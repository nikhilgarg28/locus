pub struct Meters(pub u32);

impl Meters {
    pub fn get(&self) -> u32 {
        self.0
    }
}
