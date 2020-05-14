use anyhow::{anyhow, Error};
use geojson::{GeoJson, Value, feature::Id, Geometry};
use serde_json::Value as JsonValue;
use static_bushes::{FlatBushBuilder, FlatBush};

use std::convert::TryInto;
use std::fs::File;
use std::io::{self, BufRead};

use crate::util::StringCacheBuilder;

pub struct Index {
    clusters: Vec<Cluster>,
    name_cache: Vec<String>,
    number_cache: Vec<String>,
    flatbush: FlatBush<f64>
}

pub fn load(filename: &str) -> Result<Index, Error> {
    let mut name_cache = StringCacheBuilder::new();
    let mut number_cache = StringCacheBuilder::new();
    let mut clusters = Vec::new();
    let mut builder = FlatBushBuilder::new();

    let file = File::open(filename)?;
    for (zlineno, line) in io::BufReader::new(file).lines().enumerate() {
        let lineno = zlineno + 1;
        let line = line.unwrap();
        if line.trim().len() > 0 {
            match process_row(&line, &mut name_cache, &mut number_cache) {
                Ok((cluster, bounds)) => {
                    clusters.push(cluster);
                    builder.add(bounds);
                }
                Err(e) => {
                    println!("warning: skipping {} because of {:?}", lineno, e);
                    continue;
                }
            }
        }
    }

    let flatbush = builder.finish();
    let name_cache = name_cache.finish();
    let number_cache = number_cache.finish();

    Ok(Index { clusters, name_cache, number_cache, flatbush })
}

#[derive(Debug)]
enum AddressNumber {
    U32(u32),
    String(u32)
}

#[derive(Debug)]
struct CompactAddressPoint {
    point: [f64; 2],
    number: AddressNumber
}

#[derive(Debug)]
struct Cluster {
    id: u64,
    points: Vec<CompactAddressPoint>,
    names: Vec<u32>
}

type Bounds = [f64; 4];

fn process_row(line: &str, name_cache: &mut StringCacheBuilder, number_cache: &mut StringCacheBuilder) -> Result<(Cluster, Bounds), anyhow::Error> {
    let data = line.parse::<GeoJson>().map_err(|_| anyhow!("failed to parse line"))?;
    let feat = if let GeoJson::Feature(feat) = data {
        feat
    } else {
        return Err(anyhow!("not a feature"));
    };

    // grab the ID
    let id = if let Some(Id::Number(n)) = feat.id {
        n.as_u64().unwrap_or(0)
    } else {
        0
    };

    // grab the address numbers
    let props = feat.properties.ok_or(anyhow!("feature has no properties"))?;
    let (idx, address_numbers) = if let Some(JsonValue::Array(v)) = props.get("carmen:addressnumber") {
        let (idx, entry) = v.iter().enumerate().skip_while(|(_, e)| **e == JsonValue::Null).next().ok_or(anyhow!("has no address numbers"))?;
        if let JsonValue::Array(num_vec) = entry {
            let nums = num_vec.into_iter().map(|n| {
                // might be a string, or not
                match n {
                    JsonValue::String(s) => {
                        match s.parse::<u32>() {
                            Ok(u_num) => AddressNumber::U32(u_num),
                            _ => AddressNumber::String(number_cache.get_id(s.clone()))
                        }
                    },
                    JsonValue::Number(json_num) => {
                        // maybe we can use it, maybe not
                        let num_u32: Option<u32> = json_num.as_u64().map(|n| n.try_into().ok()).flatten();
                        match num_u32 {
                            Some(u_num) => AddressNumber::U32(u_num),
                            // it's a float or negative or something, or it doesn't fit in a u32 -- stringify
                            _ => AddressNumber::String(number_cache.get_id(format!("{}", json_num)))
                        }
                    },
                    _ => AddressNumber::String(number_cache.get_id(format!("{}", n)))
                }
            });
            (idx, nums)
        } else {
            return Err(anyhow!("address number list isn't an array"));
        }
    } else {
        return Err(anyhow!("feature has no address numbers"));
    };

    // grab the multipoint geometry
    let collection = if let Some(Geometry { value: Value::GeometryCollection(collection), .. }) = feat.geometry {
        collection
    } else {
        return Err(anyhow!("line has no geometry collection"));
    };

    let point_pairs = if let Some(Geometry { value: Value::MultiPoint(mp), .. }) = collection.get(idx) {
        mp.into_iter().map(|p| {
            let mut arr = [0.0; 2];
            arr.copy_from_slice(&p);
            arr
        })
    } else {
        return Err(anyhow!("line has no multipoint geometry"));
    };

    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let points: Vec<_> = point_pairs.zip(address_numbers).map(|(point, number)| {
        if point[0] < bounds[0] { bounds[0] = point[0]; }
        if point[1] < bounds[1] { bounds[1] = point[1]; }
        if point[0] > bounds[2] { bounds[2] = point[0]; }
        if point[1] > bounds[3] { bounds[3] = point[1]; }
        CompactAddressPoint { point, number }
    }).collect();

    let names: Vec<u32> = if let Some(JsonValue::String(s)) = props.get("carmen:text") {
        s.split(",").map(|t| name_cache.get_id(t.to_string())).collect()
    } else {
        return Err(anyhow!("no valid names"));
    };

    Ok((Cluster { id, points, names }, bounds))
}

pub struct AddressPoint<'a> {
    pub point: [f64; 2],
    pub number: String,
    pub cluster_id: u64,
    pub address_position: u64,
    pub cluster_names: Vec<&'a str>
}

impl<'a> Index {
    pub fn query(&'a self, bbox: Bounds) -> impl Iterator<Item = AddressPoint> {
        self.flatbush.search_range(bbox[0], bbox[1], bbox[2], bbox[3]).map(move |id| {
            let cluster = &self.clusters[id];
            let names: Vec<&str> = cluster.names.iter().map(|name_id| self.name_cache[*name_id as usize].as_str()).collect();
            cluster.points.iter().enumerate().flat_map(move |(i, address)| {
                if address.point[0] >= bbox[0] && address.point[0] <= bbox[2] && address.point[1] >= bbox[1] && address.point[1] <= bbox[3] {
                    let number = match address.number {
                        AddressNumber::U32(num) => num.to_string(),
                        AddressNumber::String(id) => self.number_cache[id as usize].clone()
                    };
                    Some(AddressPoint {
                        point: address.point,
                        number,
                        cluster_id: cluster.id,
                        address_position: i as u64,
                        cluster_names: names.clone()
                    })
                } else {
                    None
                }
            })
        }).flatten()
    }
}
