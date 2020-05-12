use indexmap::IndexMap;

pub struct StringCacheBuilder {
    map: IndexMap<String, usize>
}

impl StringCacheBuilder {
    pub fn new() -> Self {
        Self { map: IndexMap::new() }
    }

    pub fn get_id(&mut self, s: String) -> u32 {
        let maybe_id = self.map.len();
        *(self.map.entry(s).or_insert(maybe_id)) as u32
    }

    pub fn finish(self) -> Vec<String> {
        self.map.into_iter().map(|(k, _v)| k).collect()
    }
}
