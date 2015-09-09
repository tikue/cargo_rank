#![feature(fs_walk, custom_derive, plugin, iter_arith, result_expect)]
#![plugin(serde_macros)]
extern crate serde;
extern crate serde_json;
extern crate nalgebra;

use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::Deserializer;

use nalgebra::{DMat, DVec, Iterable};

use std::io::{BufReader, BufRead};
use std::fs::{File, WalkDir, read_dir, walk_dir};
use std::env::args;
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
struct Package {
    name: String,
    vers: String,
    deps: Vec<Dep>,
    cksum: String,
    features: JsonValue,
    yanked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Dep {
    name: String,
    req: String,
    features: Vec<String>,
    optional: bool,
    default_features: bool,
    target: Option<String>,
    kind: Option<String>,
}

fn get_packages(path: &str) -> Vec<Package> {
    read_dir(path)
        .unwrap()
        .map(Result::unwrap)
        .filter(|dir| {
            let path = dir.path();
            let file = path.file_name().unwrap().to_str().unwrap();
            file != ".git" && file != "config.json"
        })
        .flat_map::<WalkDir, _>(|dir| walk_dir(dir.path()).unwrap())
        .map(Result::unwrap)
        .filter(|file| file.file_type().unwrap().is_file())
        .map(|file| {
            let reader = BufReader::new(File::open(file.path()).unwrap());
            let latest_version = reader.lines().last().unwrap().unwrap();
            let mut deserializer = Deserializer::new(latest_version.as_bytes().iter().map(|b| Ok(*b)));
            let pkg = Package::deserialize(&mut deserializer).unwrap();
            pkg
        })
        .collect()
}

fn cargo_rank(packages: &[Package]) -> Vec<(&Package, f64)> {
    let damp = 0.85;
    let starting_rank = 1.0 / packages.len() as f64;
    let mut cargo_ranks = DVec::from_elem(packages.len(), starting_rank);
    let deps: Vec<f64> = packages.iter().flat_map(|pkg| {
        let deps: HashSet<_> = pkg.deps.iter().map(|dep| &*dep.name).collect();
        let num_deps = deps.len() as f64;
        packages.iter().map(move |dep| if deps.len() == 0 { 
            1.0 / packages.len() as f64 
        } else { 
            if deps.contains(&*dep.name) { 1.0 / num_deps } else { 0.0 }
        })
    }).collect();
    let deps = DMat::from_col_vec(packages.len(), packages.len(), &deps);

    let mut delta = std::f64::MAX;
    let threshold = 0.000001;
    let starting_ranks = DVec::from_elem(packages.len(), (1.0 - damp) / packages.len() as f64);
    while delta > threshold {
        let new_ranks = starting_ranks.clone() + deps.clone() * cargo_ranks.clone() * damp;
        delta = cargo_ranks.iter().zip(new_ranks.iter()).map(|(old, new)| (old - new).abs()).sum();
        println!("Delta: {}", delta);
        cargo_ranks = new_ranks;
    }
    let mut ranks: Vec<_> = packages.iter().zip(cargo_ranks.iter().cloned()).collect();
    ranks.sort_by(|&(_, rank1), &(_, rank2)| rank1.partial_cmp(&rank2).unwrap().reverse());
    ranks
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let packages = get_packages(&path);
    let limit = args.next().unwrap().parse().expect("Not a number?");

    println!("ranks:");
    let ranks = cargo_rank(&packages);
    for (i, (pkg, rank)) in ranks.into_iter().take(limit).enumerate() {
        println!("{}. {} ({})", i + 1, pkg.name, rank);
    }
}
