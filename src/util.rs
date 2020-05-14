use indexmap::IndexMap;

use std::collections::hash_map::DefaultHasher;
use std::f64::consts::PI;
use std::hash::{Hash, Hasher};

const WEB_MERC_CONST: f64 = 20037508.3427892;

#[allow(dead_code)]
pub fn wgs84_to_web_mercator(point: [f64; 2]) -> [f64; 2] {
    let x: f64 = point[0] * WEB_MERC_CONST / 180.0;
    let mut y: f64 = (((90.0 + point[1]) * PI / 360.0).tan()).ln() / (PI / 180.0);

    y = y * WEB_MERC_CONST / 180.0;
    [x, y]
}

pub fn web_mercator_to_wgs84(point: [f64; 2]) -> [f64; 2] {
    let lon: f64 = (point[0] / WEB_MERC_CONST) * 180.0;
    let mut lat: f64 = (point[1] / WEB_MERC_CONST) * 180.0;

    lat = 180.0/PI * (2.0 * ((lat * PI / 180.0).exp()).atan() - PI / 2.0);
    [lon, lat]
}

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

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
