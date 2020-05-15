# Triodia

Triodia is a hacky partial implementation of an alternative backend for [Hecate](https://github.com/mapbox/Hecate) intended for ephemeral visualization and spatial querying of line-delimited GeoJSON data in [Carmen](https://github.com/mapbox/carmen) address cluster format. Unlike Hecate, it doesn't use a database or other persistent storage, and instead builds a very compact in-memory spatial index on the fly at run-time, and then makes a limited subset of the Hecate API available on top of the data and index once complete. It also serves the Hecate UI, which is somewhat broken since not all endpoints are implemented, but it works well enough to scroll around and click on some points.

## Caveats

* The only functioning endpoints are the tile endpoint (`/api/tiles/{z}/{x}/{y}`) and the multiple-features endpoint that supports spatial querying (`/api/data/features`, plus either a `bbox` or `point` parameter). These behave as [documented in Hecate](https://github.com/mapbox/Hecate/blob/master/README.md#api), are used by the Hecate UI, and can be used programmatically if you want to spatially query the addresses you're working with.
* The only metadata from the GeoJSON that's exposed per address point, either in the API or the UI, is the ID of the address cluster it came from, the name or names of the street, and the house number. Triodia doesn't retain the original source GeoJSON as it ingests data in order to keep the indexes small enough to fit in memory, and at this point that's all it keeps.
* Speaking of: depending on the size of the data, it can be pretty memory-hungry, though typically it uses much less memory than the size of the original GeoJSON (somewhere between a quarter half as much)
* As per above: the Hecate UI is served unmodified from upstream Hecate, so there are buttons there for things that aren't supported upstream, and they tend to show error messages if you click them.
* Only address points are served, not interpolation lines or any other geometry.
* **This is an unsupported hack that is what it is. There are no long-term plans at this time.**

## How to use it

Triodia requires current Rust stable to install, as well as Node and yarn.

To install:
```
git clone git@github.com:apendleton/triodia.git
cd triodia
cargo build --release
cd web
yarn install && yarn build
cd ..
```

To serve some data:
```
cargo run --release local-path-to-my-geojson.geojson
```
It'll think for awhile but eventually say `starting server on port 9005...` and then you're good to go.

To view your data, visit `http://localhost:9005/admin`.

Tip: if you want to serve data from multiple indexes, you can just `cat` their respective GeoJSON files together into one file and serve that.

Relevant links:
* [Hecate](https://github.com/mapbox/Hecate) obviously
* [static-bushes](https://github.com/apendleton/static-bushes), my WIP Rust port of a couple of @mourner's [kdbush](https://github.com/mourner/kdbush) and [flatbush](https://github.com/mourner/flatbush/) JS libraries (flatbush is the one in use here)
