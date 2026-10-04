//! Content-free, direct Actor timing with the installed Broker's session options.
//! Run only with a disposable, marker-guarded Rime user directory.
//! Compiled fixtures may live in user/build or a separate guarded machine directory.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_domain::{EngineCommand, KeyEvent, SessionOptions};
    use mo_engine::EngineActor;
    use mo_rime::{Engine, EngineConfig, RimeBackend};
    use std::{io, path::PathBuf, time::Instant};

    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let invocation = arguments::parse(&args)?;
    invocation.validate()?;
    let [dll, shared, user] = [&invocation.dll, &invocation.shared, &invocation.user];
    let broker_plan = invocation.broker_plan;
    let unicode = |path: &std::path::Path| -> Result<String, io::Error> {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "paths must be Unicode"))
    };
    let started = Instant::now();
    let mut config = EngineConfig::new(unicode(shared)?, unicode(user)?);
    if let Some(prebuilt) = &invocation.prebuilt {
        let path = unicode(prebuilt)?;
        config.prebuilt_data_dir = Some(path.clone());
        config.staging_dir = Some(path);
    }
    let engine = Engine::load(config, dll)?;
    let load_us = started.elapsed().as_micros();
    let started = Instant::now();
    let mut actor = EngineActor::new(RimeBackend::with_prepared_resources(engine)?);
    let prepare_us = started.elapsed().as_micros();
    println!(
        "MO_ACTOR_READY load_us={load_us} prepare_us={prepare_us} input_free=true shared_prebuilt={}",
        invocation.prebuilt.is_some()
    );

    for trial in 0..2 {
        let mut options = SessionOptions::new();
        if broker_plan {
            options = options
                .with_schema("rime_ice")
                .with_option("traditionalization", false)
                .with_option("emoji", true)
                .with_option("mo_disable_learning", true);
        }
        let started = Instant::now();
        let token = actor.create_session(options)?;
        let create_us = started.elapsed().as_micros();
        let started = Instant::now();
        let snapshot = actor.dispatch(token, EngineCommand::Key(KeyEvent::text('n')))?;
        let dispatch_us = started.elapsed().as_micros();
        assert!(snapshot.handled && snapshot.commit.is_none());
        assert_eq!(snapshot.status.schema_id, "rime_ice");
        assert!(snapshot.status.composing);
        println!(
            "MO_ACTOR trial={trial} broker_plan={broker_plan} create_us={create_us} dispatch_us={dispatch_us} candidate_count={}",
            snapshot.candidates.len()
        );
        let cleared = actor.dispatch(token, EngineCommand::Clear)?;
        assert!(cleared.commit.is_none());
        actor.destroy_session(token)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("actor_latency_probe currently requires Windows");
    std::process::exit(1);
}

#[cfg(any(windows, test))]
mod arguments {
    use std::{io, path::PathBuf};

    pub struct Invocation {
        pub dll: PathBuf,
        pub shared: PathBuf,
        pub user: PathBuf,
        pub broker_plan: bool,
        pub prebuilt: Option<PathBuf>,
    }

    fn invalid() -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: actor_latency_probe <absolute-rime.dll> <absolute-shared> <absolute-disposable-user> [--broker-plan] [--machine-prebuilt <absolute-fixture-dir>]",
        )
    }

    pub fn parse(args: &[PathBuf]) -> io::Result<Invocation> {
        let [dll, shared, user, flags @ ..] = args else {
            return Err(invalid());
        };
        let mut invocation = Invocation {
            dll: dll.clone(),
            shared: shared.clone(),
            user: user.clone(),
            broker_plan: false,
            prebuilt: None,
        };
        let mut index = 0;
        while index < flags.len() {
            if flags[index] == std::path::Path::new("--broker-plan") && !invocation.broker_plan {
                invocation.broker_plan = true;
                index += 1;
            } else if flags[index] == std::path::Path::new("--machine-prebuilt")
                && invocation.prebuilt.is_none()
                && index + 1 < flags.len()
            {
                invocation.prebuilt = Some(flags[index + 1].clone());
                index += 2;
            } else {
                return Err(invalid());
            }
        }
        Ok(invocation)
    }

    impl Invocation {
        pub fn validate(&self) -> io::Result<()> {
            if [&self.dll, &self.shared, &self.user]
                .iter()
                .any(|path| !path.is_absolute())
                || !self.user.join("mo-latency-fixture").is_file()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "absolute paths and a disposable mo-latency-fixture marker are required",
                ));
            }
            if let Some(prebuilt) = &self.prebuilt {
                if !prebuilt.is_absolute()
                    || [
                        "mo-latency-prebuilt-fixture",
                        "default.yaml",
                        "rime_ice.schema.yaml",
                    ]
                    .iter()
                    .any(|name| !prebuilt.join(name).is_file())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "machine prebuilt data requires an absolute compiled fixture and marker",
                    ));
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn args(flags: &[&str]) -> Vec<PathBuf> {
            ["dll", "shared", "user"]
                .into_iter()
                .chain(flags.iter().copied())
                .map(PathBuf::from)
                .collect()
        }

        #[test]
        fn legacy_invocation_and_independent_prebuilt_options_are_supported() {
            assert!(parse(&args(&[])).unwrap().prebuilt.is_none());
            let legacy = parse(&args(&["--broker-plan"])).unwrap();
            assert!(legacy.broker_plan && legacy.prebuilt.is_none());
            for flags in [
                ["--broker-plan", "--machine-prebuilt", "fixture"],
                ["--machine-prebuilt", "fixture", "--broker-plan"],
            ] {
                let invocation = parse(&args(&flags)).unwrap();
                assert!(invocation.broker_plan);
                assert_eq!(invocation.prebuilt, Some(PathBuf::from("fixture")));
            }
        }

        #[test]
        fn malformed_or_duplicate_options_are_rejected_before_loading_native_code() {
            for flags in [
                vec!["--unknown"],
                vec!["--machine-prebuilt"],
                vec!["--broker-plan", "--broker-plan"],
                vec!["--machine-prebuilt", "one", "--machine-prebuilt", "two"],
            ] {
                assert!(parse(&args(&flags)).is_err());
            }
            assert!(parse(&[]).is_err());
            assert!(parse(&args(&[])).unwrap().validate().is_err());
        }

        #[test]
        fn compiled_prebuilt_and_user_markers_are_both_required() -> io::Result<()> {
            let root = std::env::temp_dir().join(format!(
                "mo-actor-prebuilt-guard-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&root)?;
            let user = root.join("user");
            let prebuilt = root.join("prebuilt");
            std::fs::create_dir(&user)?;
            std::fs::create_dir(&prebuilt)?;
            let invocation = Invocation {
                dll: root.join("rime.dll"),
                shared: root.join("shared"),
                user: user.clone(),
                broker_plan: true,
                prebuilt: Some(prebuilt.clone()),
            };
            assert!(invocation.validate().is_err());
            std::fs::write(user.join("mo-latency-fixture"), [])?;
            assert!(invocation.validate().is_err());
            for name in ["default.yaml", "rime_ice.schema.yaml"] {
                std::fs::write(prebuilt.join(name), [])?;
            }
            assert!(invocation.validate().is_err());
            std::fs::write(prebuilt.join("mo-latency-prebuilt-fixture"), [])?;
            invocation.validate()?;
            std::fs::remove_file(user.join("mo-latency-fixture"))?;
            assert!(invocation.validate().is_err());
            for name in [
                "mo-latency-prebuilt-fixture",
                "default.yaml",
                "rime_ice.schema.yaml",
            ] {
                std::fs::remove_file(prebuilt.join(name))?;
            }
            std::fs::remove_dir(prebuilt)?;
            std::fs::remove_dir(user)?;
            std::fs::remove_dir(root)?;
            Ok(())
        }
    }
}
