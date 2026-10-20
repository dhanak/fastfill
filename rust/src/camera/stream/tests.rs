use super::*;

#[test]
fn watchdog_kills_and_reaps_stalled_or_canceled_children() {
    for cancel in [false, true] {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "camera::tests::worker_waits_only_in_timeout_fixture",
            ])
            .env("FASTFILL_CAMERA_PROBE_TEST_WORKER", "wait")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let child = Arc::new(Mutex::new(Some(child)));
        let failure = Mutex::new(None);
        let start = Instant::now();
        supervise(
            Process(child.clone()),
            &AtomicBool::new(cancel),
            &Mutex::new(start),
            &failure,
            Duration::from_millis(30),
            SystemTime::now,
        );
        assert!(child.lock().unwrap().is_none());
        assert_eq!(failure.lock().unwrap().is_some(), !cancel);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn watchdog_interrupts_on_sleep_or_clock_rollback_without_waiting() {
    for jump_forward in [true, false] {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "camera::tests::worker_waits_only_in_timeout_fixture",
            ])
            .env("FASTFILL_CAMERA_PROBE_TEST_WORKER", "wait")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let child = Arc::new(Mutex::new(Some(child)));
        let failure = Mutex::new(None);
        let start = Instant::now();
        let mut clock = SystemTime::now();
        supervise(
            Process(child.clone()),
            &AtomicBool::new(false),
            &Mutex::new(start),
            &failure,
            Duration::from_secs(20),
            move || {
                let previous = clock;
                clock = if jump_forward {
                    clock + Duration::from_secs(60)
                } else {
                    clock - Duration::from_secs(60)
                };
                previous
            },
        );
        assert!(child.lock().unwrap().is_none());
        assert_eq!(failure.lock().unwrap().as_deref(), Some(INTERRUPTED));
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn dropping_stream_requests_immediate_termination() {
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "camera::tests::worker_waits_only_in_timeout_fixture",
        ])
        .env("FASTFILL_CAMERA_PROBE_TEST_WORKER", "wait")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let child = Arc::new(Mutex::new(Some(child)));
    let canceled = Arc::new(AtomicBool::new(false));
    drop(Stream {
        canceled: canceled.clone(),
        child: child.clone(),
    });
    assert!(canceled.load(Ordering::Relaxed));
    assert!(
        !child
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
}

#[test]
fn malformed_worker_arguments_never_reach_camera_backend() {
    for args in [
        vec![],
        vec![""],
        vec!["-1"],
        vec!["0"],
        vec!["0", "extra"],
        vec!["0", "", "480", "30", "MJPG"],
        vec!["0", "640", "0", "30", "MJPG"],
        vec!["0", "640", "480", "0", "MJPG"],
        vec!["0", "640", "480", "30", "H264"],
        vec!["0", "640", "480", "30", "MJPG", "extra"],
    ] {
        assert!(
            worker(&args.iter().map(OsString::from).collect::<Vec<_>>())
                .is_err()
        );
    }
}

#[test]
fn catalog_worker_fixture() {
    match std::env::var("FASTFILL_CATALOG_TEST_WORKER").as_deref() {
        Ok("wait") => std::thread::sleep(Duration::from_secs(10)),
        Ok("large") => {
            let length = usize::try_from(super::super::catalog::LIMIT).unwrap();
            let bytes = vec![b'x'; length + 1];
            let _ = std::io::stdout().write_all(&bytes);
        }
        Ok("ok") => println!("synthetic camera catalog"),
        Ok("noisy") => {
            std::io::stderr()
                .write_all(&vec![b'x'; ERROR_LIMIT as usize * 2])
                .unwrap();
            println!("synthetic camera catalog");
        }
        Ok("failed") => {
            eprintln!("virtual camera is offline");
            std::process::exit(7);
        }
        _ => (),
    }
}

#[test]
fn catalog_reader_bounds_output_and_kills_stalled_workers() {
    for mode in ["ok", "noisy", "failed", "large", "wait"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "camera::stream::tests::catalog_worker_fixture",
                "--nocapture",
            ])
            .env("FASTFILL_CATALOG_TEST_WORKER", mode);
        let start = Instant::now();
        let (sender, receiver) = std::sync::mpsc::channel();
        let query = start_query(
            command,
            if mode == "wait" {
                Duration::from_millis(30)
            } else {
                Duration::from_secs(5)
            },
            move |result| {
                let _ = sender.send(result);
            },
        )
        .unwrap();
        let result = receiver.recv_timeout(Duration::from_secs(6)).unwrap();
        drop(query);
        match mode {
            "ok" | "noisy" => assert!(
                String::from_utf8_lossy(&result.unwrap())
                    .contains("synthetic camera catalog")
            ),
            "large" => assert!(result.unwrap_err().contains("oversized")),
            "failed" => {
                let error = result.unwrap_err();
                assert!(error.contains('7'), "{error}");
                assert!(error.contains("virtual camera is offline"), "{error}");
            }
            _ => assert!(result.unwrap_err().contains("timed out")),
        }
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn empty_discovery_reply_is_not_reported_as_an_oversized_catalog() {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("cmd.exe");
    #[cfg(target_os = "windows")]
    command.args(["/D", "/C", "exit /B 0"]);
    #[cfg(not(target_os = "windows"))]
    let mut command = Command::new("/bin/sh");
    #[cfg(not(target_os = "windows"))]
    command.args(["-c", "exit 0"]);
    let (sender, receiver) = std::sync::mpsc::channel();
    let _query = start_query(command, Duration::from_secs(5), move |result| {
        sender.send(result).unwrap();
    })
    .unwrap();
    let error = receiver
        .recv_timeout(Duration::from_secs(6))
        .unwrap()
        .unwrap_err();
    assert!(error.contains("no data"), "{error}");
    assert!(!error.contains("oversized"), "{error}");
}

#[test]
fn dropping_discovery_kills_and_reaps_without_delivering_a_result() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "camera::stream::tests::catalog_worker_fixture",
            "--nocapture",
        ])
        .env("FASTFILL_CATALOG_TEST_WORKER", "wait");
    let (sender, receiver) = std::sync::mpsc::channel();
    let query = start_query(command, Duration::from_secs(20), move |result| {
        let _ = sender.send(result);
    })
    .unwrap();
    let child = query.child.clone();
    drop(query);
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(5)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(child.lock().unwrap().is_none());
}
