use anyhow::{anyhow, Error};
use geojson::{GeoJson, Value, feature::Id, Geometry};
use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use static_bushes::FlatBushBuilder;

use std::env;
use std::fs::File;
use std::io::{self, BufRead};

fn main() {
    start().unwrap();
}

fn start() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();

    let mut name_cache: IndexMap<String, usize> = IndexMap::new();
    let mut clusters = Vec::new();
    let mut builder = FlatBushBuilder::new();

    let filename = args.get(1).expect("must provide one filename");
    let file = File::open(filename)?;
    for (zlineno, line) in io::BufReader::new(file).lines().enumerate() {
        let lineno = zlineno + 1;
        let line = line.unwrap();
        if line.trim().len() > 0 {
            match process_row(&line, &mut name_cache) {
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

    let bush = builder.finish();
    let name_cache: Vec<String> = name_cache.into_iter().map(|(k, _v)| k).collect();

    Ok(())
}

#[derive(Debug)]
enum AddressNumber {
    U64(u64),
    String(String)
}

#[derive(Debug)]
struct AddressPoint {
    point: [f64; 2],
    number: AddressNumber
}

#[derive(Debug)]
struct Cluster {
    id: u64,
    points: Vec<AddressPoint>,
    names: Vec<usize>
}

type Bounds = [f64; 4];

fn process_row(line: &str, name_cache: &mut IndexMap<String, usize>) -> Result<(Cluster, Bounds), anyhow::Error> {
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
                        match s.parse::<u64>() {
                            Ok(u_num) => AddressNumber::U64(u_num),
                            _ => AddressNumber::String(s.clone())
                        }
                    },
                    JsonValue::Number(json_num) => {
                        // maybe we can use it, maybe not
                        match json_num.as_u64() {
                            Some(u_num) => AddressNumber::U64(u_num),
                            // it's a float or negative or something -- stringify
                            _ => AddressNumber::String(format!("{}", json_num))
                        }
                    },
                    _ => AddressNumber::String(format!("{}", n))
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
        AddressPoint { point, number }
    }).collect();

    let names: Vec<usize> = if let Some(JsonValue::String(s)) = props.get("carmen:text") {
        s.split(",").map(|t| {
            let maybe_id = name_cache.len();
            *(name_cache.entry(t.to_string()).or_insert(maybe_id))
        }).collect()
    } else {
        return Err(anyhow!("no valid names"));
    };

    Ok((Cluster { id, points, names }, bounds))
}
