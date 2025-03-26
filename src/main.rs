use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

use anyhow::Context;
use cargo_toml::{Dependency, Manifest};
use clap::Parser;
use dependabot_config::v2::{
    Dependabot, Ignore, Interval, PackageEcosystem, Schedule, Update, UpdateType,
};
use semver::VersionReq;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum UpdateStrategy {
    Major,
    Minor,
    Patch,
}

#[derive(Parser, Debug)]
struct Opts {
    /// Write result to .github/dependabot.yml
    #[arg(short, long)]
    write: bool,
    /// Path to the repository
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let cargo = opts.path.join("Cargo.toml");
    let m = Manifest::from_path(&cargo)
        .with_context(|| format!("Failed to parse: {}", cargo.display()))?;

    let manifests = if let Some(w) = m.workspace {
        w.members
            .iter()
            .map(|m| {
                let path = opts.path.join(format!("./{}/Cargo.toml", m));
                Manifest::from_path(&path)
                    .with_context(|| format!("Failed to parse: {}", path.display()))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![m]
    };

    let mut deps: BTreeMap<String, UpdateStrategy> = BTreeMap::new();
    for m in manifests {
        for (name, dep) in m
            .dependencies
            .iter()
            .chain(m.dev_dependencies.iter())
            .chain(m.build_dependencies.iter())
        {
            if let Dependency::Detailed(detail) = &dep {
                if detail.path.is_some() {
                    continue;
                }
            }
            let req = VersionReq::parse(dep.try_req()?)?;
            let comp = &req.comparators[0];
            let strategy = if comp.major > 0 {
                UpdateStrategy::Major
            } else if let Some(minor) = comp.minor {
                if minor > 0 {
                    UpdateStrategy::Minor
                } else {
                    UpdateStrategy::Patch
                }
            } else {
                UpdateStrategy::Patch
            };
            deps.insert(name.clone(), strategy);
        }
    }

    let mut dependabot_path = opts.path.join(".github/dependabot.yaml");
    if !dependabot_path.exists() {
        dependabot_path = dependabot_path.with_extension("yml")
    }

    let mut config = if let Ok(config) = std::fs::read_to_string(&dependabot_path) {
        Dependabot::from_str(&config).with_context(|| {
            format!(
                "Failed to parse dependabot config from {}",
                dependabot_path.display()
            )
        })?
    } else {
        Dependabot::new(vec![])
    };

    let mut update = config
        .updates
        .iter()
        .find(|u| u.package_ecosystem == PackageEcosystem::Cargo)
        .cloned()
        .unwrap_or(Update::new(
            dependabot_config::v2::PackageEcosystem::Cargo,
            "/",
            Schedule::new(Interval::Weekly),
        ));

    let mut ignores: Vec<_> = deps
        .iter()
        .filter_map(|(name, strategy)| {
            if *strategy == UpdateStrategy::Minor {
                None
            } else {
                let update_types = match strategy {
                    UpdateStrategy::Major => {
                        vec![UpdateType::SemverMinor, UpdateType::SemverPatch]
                    }
                    UpdateStrategy::Minor => {
                        vec![UpdateType::SemverPatch]
                    }
                    UpdateStrategy::Patch => return None,
                };
                let mut ignore = Ignore::new(name.clone());
                ignore.update_types = Some(update_types);
                Some(ignore)
            }
        })
        .collect();

    let mut default_ignore = Ignore::new(String::from("*"));
    default_ignore.update_types = Some(vec![UpdateType::SemverPatch]);
    ignores.push(default_ignore);

    update.ignore = Some(ignores);

    if let Some(pos) = config
        .updates
        .iter()
        .position(|u| u.package_ecosystem == PackageEcosystem::Cargo)
    {
        config.updates[pos] = update;
    } else {
        config.updates.push(update);
    }

    if opts.write {
        println!("Writing to {}", dependabot_path.display());
        let dependabot_dir = dependabot_path.parent().unwrap();
        std::fs::create_dir_all(dependabot_dir).with_context(|| {
            format!("Failed to create directories: {}", dependabot_dir.display())
        })?;
        std::fs::write(&dependabot_path, config.to_string()).with_context(|| {
            format!(
                "Failed to write dependabot config to {}",
                dependabot_path.display()
            )
        })?;
    } else {
        println!("{}", config.to_string());
    }

    Ok(())
}
