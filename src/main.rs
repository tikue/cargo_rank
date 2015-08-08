#![feature(fs_walk, custom_derive, plugin, iter_arith, convert)]
#![plugin(serde_macros)]
extern crate serde;
extern crate serde_json;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::Deserializer;

use std::io::{BufReader, BufRead};
use std::fs::{File, WalkDir, read_dir, walk_dir};
use std::env::args;
use std::collections::HashMap;
use std::mem::swap;

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
    let mut cargo_ranks: HashMap<_, _> = packages.iter().map(|pkg| (pkg.name.as_str(), (pkg, starting_rank))).collect();
    let mut new_ranks: HashMap<_, _>;
    let mut delta = std::f64::MAX;
    let threshold = 0.000001;
    let iterative_starting_rank = (1.0 - damp) / packages.len() as f64;
    while delta > threshold {
        new_ranks = packages.iter().map(|pkg| (&pkg.name[..], (pkg, iterative_starting_rank))).collect();
        for &(pkg, rank) in cargo_ranks.values() {
            let num_deps = pkg.deps.len();
            let boost;
            if num_deps == 0 { // distribute evenly when there are no deps
                boost = damp * rank / (packages.len() as f64 - 1.0);
                for dep in packages.iter().filter(|dep| dep.name != pkg.name) {
                    new_ranks.get_mut(dep.name.as_str()).unwrap().1 += boost;
                }
            } else {
                boost = damp * rank / num_deps as f64;
                for dep in &pkg.deps {
                    (new_ranks.get_mut(dep.name.as_str())).unwrap().1 += boost;
                }
            }
        }
        delta = cargo_ranks.values().map(|&(ref pkg, ref rank)| (new_ranks[pkg.name.as_str()].1 - rank).abs()).sum();
        cargo_ranks.clear();
        swap(&mut cargo_ranks, &mut new_ranks);
        println!("Delta: {}", delta);
    }
    let mut ranks: Vec<_> = cargo_ranks.into_iter().map(|(_, pkg_rank)| pkg_rank).collect();
    ranks.sort_by(|&(_, rank1), &(_, ref rank2)| rank1.partial_cmp(rank2).unwrap().reverse());
    ranks
}

fn main() {
    let path = std::env::args().skip(1).next().unwrap();
    let packages = get_packages(&path);
    let ranks = cargo_rank(&packages);

    for (pkg, rank) in ranks.into_iter().take(10) {
        println!("{}: {}", pkg.name, rank);
    }
}
