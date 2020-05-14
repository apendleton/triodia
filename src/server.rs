use actix_web::{web, web::Json, App, HttpResponse, HttpServer, middleware, Responder, error::ErrorBadRequest};
use anyhow::anyhow;
use flat_projection::FlatProjection;
use mvt::{GeomEncoder, GeomType, Tile, Transform};
use ordered_float::OrderedFloat;
use serde::Deserialize;
use serde_json::{self, json};
use tile_grid::{Origin, Grid};

use std::sync::Arc;

use crate::index::Index;
use crate::util::{calculate_hash, web_mercator_to_wgs84};

pub static VERSION: &str = env!("CARGO_PKG_VERSION");
pub static MAX_BODY: u64 = 20_971_520;

pub fn start(store: Arc<Index>, port: Option<u16>) -> actix_server::Server {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::NormalizePath)
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .data(store.clone())
            //TODO HANDLE GENERIC 404
            .route("/", web::get().to(index))
            .service(
                actix_files::Files::new("/admin", "./web/dist/")
                    .index_file("index.html")
            )
            .service(web::scope("api")
                .service(web::resource("")
                    .route(web::get().to(server))
                )
                .service(web::resource("auth")
                    .route(web::get().to(auth))
                )
                .service(web::resource("meta/layers")
                    .route(web::get().to(meta_layers))
                )
                .service(web::resource("schema")
                    .route(web::get().to(schema))
                )
                .service(web::resource("tiles/{z}/{x}/{y}")
                    .route(web::get().to(tile))
                )
                .service(web::resource("data/features")
                    .route(web::get().to(features_query))
                )
                .service(web::resource("user/info")
                    .route(web::get().to(user_info))
                )
            )
    })
        .bind(format!("0.0.0.0:{}", port.unwrap_or(8000)).as_str())
        .unwrap()
        .run()
}

async fn index() -> &'static str { "Hello World!" }

async fn server() -> impl Responder {
    Json(json!({
        "version": VERSION,
        "constraints": {
            "request": {
                "max_size": MAX_BODY
            }
        }
    }))
}

async fn user_info() -> impl Responder {
    Json(json!({
        "access": "default",
        "email": "andrew@mapbox.com",
        "id": 11405,
        "meta": {},
        "username": "andrew"
    }))
}

async fn meta_layers() -> impl Responder {
    Json(json!([
        {
            "name": "Mapbox Satellite",
            "type": "Raster",
            "url": "mapbox://styles/mapbox/satellite-streets-v10"
        },
        {
            "name": "Mapbox Streets",
            "type": "Vector",
            "url": "mapbox://styles/mapbox/streets-v11"
        },
        {
            "name": "Mapbox Light",
            "type": "Vector",
            "url": "mapbox://styles/mapbox/light-v9"
        }
    ]))
}

async fn auth() -> impl Responder {
    Json(json!({
        "auth": {
            "get": "public"
        },
        "default": "user",
        "feature": {
            "get": "public"
        },
        "meta": {
            "get": "public"
        },
        "mvt": {
            "get": "public"
        },
        "osm": {
            "create": "user",
            "get": "public"
        },
        "schema": {
            "get": "public"
        },
        "server": "public",
        "user": {
            "create": "admin",
            "create_session": "self",
            "info": "self",
            "list": "user"
        }
    }))
}

async fn schema() -> impl Responder {
    let schema: &'static str = include_str!("../data/schema.json");
    let json_val: serde_json::Value = serde_json::from_str(schema).unwrap();
    Json(json_val)
}

async fn tile(
    store: web::Data<Arc<Index>>,
    path: web::Path<(u8, u32, u32)>
) -> impl Responder {
    let res = web::block(move || {
        let z = path.0;
        let x = path.1;
        let y = path.2;

        if z > 17 { return Err(anyhow!("zoom must be <= 17")); }



        let mut tile = Tile::new(4096);
        let mut layer = tile.create_layer("data");

        let mut grid = Grid::web_mercator();
        grid.origin = Origin::TopLeft;

        let extent = grid.tile_extent(x, y, z);
        let [minx, miny] = web_mercator_to_wgs84([extent.minx, extent.miny]);
        let [maxx, maxy] = web_mercator_to_wgs84([extent.maxx, extent.maxy]);

        if z > 10 {
            for item in store.query([minx, miny, maxx, maxy]) {
                let tile_x = (4096.0 * (item.point[0] - minx) / (maxx - minx)).round();
                let tile_y = (4096.0 * (maxy - item.point[1]) / (maxy - miny)).round();

                let pt = GeomEncoder::new(GeomType::Point, Transform::new()).point(tile_x, tile_y).encode();
                let pt = pt?;

                let mut feature = layer.into_feature(pt);
                feature.set_id(0);
                layer = feature.into_layer();
            }
        }

        tile.add_layer(layer)?;
        let tile = tile.to_bytes()?;

        Ok(tile)
    }).await;

    match res {
        Ok(tile) => HttpResponse::build(actix_web::http::StatusCode::OK)
            .content_type("application/x-protobuf")
            .content_length(tile.len() as u64)
            .body(tile),
        Err(_) => HttpResponse::InternalServerError().into()
    }
}

#[derive(Deserialize, Debug)]
struct FeaturesParams {
    bbox: Option<String>,
    point: Option<String>
}
async fn features_query(
    store: web::Data<Arc<Index>>,
    map: web::Query<FeaturesParams>
) -> impl Responder {
    let (bbox, proj_info) = if map.bbox.is_some() && map.point.is_some() {
        return HttpResponse::from_error(ErrorBadRequest("key and point params cannot be used together"));
    } else if let Some(box_param) = &map.bbox {
        let bbox: Vec<f64> = box_param.split(',').map(|s| s.parse().unwrap()).collect();
        if bbox.len() >= 4 {
            ([bbox[0], bbox[1], bbox[2], bbox[3]], None)
        } else {
            return HttpResponse::from_error(ErrorBadRequest("bbox must have four coords"));
        }
    } else if let Some(point_param) = &map.point {
        let pt: Vec<f64> = point_param.split(',').map(|s| s.parse().unwrap()).collect();

        if pt.len() < 2 {
            return HttpResponse::from_error(ErrorBadRequest("pt must have two coords"));
        }

        let proj = FlatProjection::new(pt[0], pt[1]);
        let center = proj.project(pt[0], pt[1]);

        // 5 meters to each side
        let min_corner = proj.unproject(&center.offset(-0.005, -0.005));
        let max_corner = proj.unproject(&center.offset(0.005, 0.005));

        ([min_corner.0, min_corner.1, max_corner.0, max_corner.1], Some((proj, center)))
    } else {
        return HttpResponse::from_error(ErrorBadRequest("key or point param must be used"));
    };

    let res: Result<_, actix_threadpool::BlockingError<()>> = web::block(move || {
        let query = store.query(bbox);
        let matches = if let Some((proj, center)) = proj_info {
            let mut match_vec: Vec<_> = query.map(|item| {
                let mpt = proj.project(item.point[0], item.point[1]);
                (item, OrderedFloat(mpt.distance_squared(&center)))
            }).collect();
            match_vec.sort_by_key(|a| a.1);
            match_vec
        } else {
            query.map(|item| (item, OrderedFloat(0.0))).collect()
        };

        let matches_as_json = matches.iter().map(|(item, _)| {
            let streets: Vec<_> = item.cluster_names.iter().enumerate().map(|(i, name)| json!({
                "display": name,
                "priority": i
            })).collect();

            let record = json!({
                "id": calculate_hash(&(item.cluster_id, item.address_position)),
                "key": null,
                "type": "Feature",
                "version": 1,
                "geometry": {
                    "type": "Point",
                    "coordinates": item.point
                },
                "properties": {
                    "number": item.number,
                    "street": streets,
                    "cluster_id": item.cluster_id
                }
            });

            record.to_string()
        });

        Ok(itertools::join(matches_as_json.chain(std::iter::once("\x04".to_string())), "\n"))
    }).await;


    match res {
        Ok(rows) => HttpResponse::build(actix_web::http::StatusCode::OK)
            .content_type("application/x-ndjson")
            .content_length(rows.len() as u64)
            .body(rows),
        Err(_) => HttpResponse::InternalServerError().into()
    }
}
