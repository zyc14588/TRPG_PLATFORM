#[test]
fn production_agent_job_kill9_child() {
    let now_unix_ms = match env::var("AR09_PRODUCTION_KILL9_NOW") {
        Ok(value) => Some(
            value
                .parse::<i64>()
                .expect("production kill9 child time must be numeric"),
        ),
        Err(env::VarError::NotPresent) => {
            assert!(
                env::var("AR09_PRODUCTION_KILL9_BOUNDARY").is_err()
                    && env::var("AR09_PRODUCTION_KILL9_CHARACTER_ID").is_err()
                    && env::var("AR09_PRODUCTION_KILL9_NAMESPACE").is_err()
                    && env::var("AR09_PRODUCTION_KILL9_AUDIT_PATH").is_err(),
                "production kill9 child configuration must be complete"
            );
            None
        }
        Err(env::VarError::NotUnicode(_)) => {
            panic!("production kill9 child time must be valid Unicode")
        }
    };
    if let Some(now_unix_ms) = now_unix_ms {
        let (_setup_runtime, worker) = production_worker_from_environment();
        let worker_runtime =
            tokio::runtime::Runtime::new().expect("create production kill9 worker runtime");
        let outcome = worker_runtime
            .block_on(worker.run_once(now_unix_ms))
            .expect("production kill9 worker execution succeeds");
        assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
    }
}

fn spawn_child(
    boundary: &str,
    now_unix_ms: i64,
    character_id: &str,
    namespace: &str,
    audit_path: &PathBuf,
) -> std::process::Child {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_PRODUCTION_KILL9_BOUNDARY", boundary)
        .env("AR09_PRODUCTION_KILL9_NOW", now_unix_ms.to_string())
        .env("AR09_PRODUCTION_KILL9_CHARACTER_ID", character_id)
        .env("AR09_PRODUCTION_KILL9_NAMESPACE", namespace)
        .env("AR09_PRODUCTION_KILL9_AUDIT_PATH", audit_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("production kill9 child must start")
}

fn recover_child(
    now_unix_ms: i64,
    character_id: &str,
    namespace: &str,
    audit_path: &PathBuf,
) -> std::process::Output {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_PRODUCTION_KILL9_BOUNDARY", "none")
        .env("AR09_PRODUCTION_KILL9_NOW", now_unix_ms.to_string())
        .env("AR09_PRODUCTION_KILL9_CHARACTER_ID", character_id)
        .env("AR09_PRODUCTION_KILL9_NAMESPACE", namespace)
        .env("AR09_PRODUCTION_KILL9_AUDIT_PATH", audit_path)
        .output()
        .expect("production kill9 recovery child must start")
}
