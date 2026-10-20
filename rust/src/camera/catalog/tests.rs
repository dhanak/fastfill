use super::*;

#[test]
fn empty_modes_and_driver_errors_are_valid_nonempty_protocol_replies() {
    assert_eq!(decode::<Spec>(Ok(br#"{"Ok":[]}"#.to_vec())).unwrap(), []);
    assert_eq!(
        decode::<Spec>(Ok(br#"{"Err":"Camera is offline"}"#.to_vec()))
            .unwrap_err(),
        "Camera is offline",
    );
    assert!(decode::<Spec>(Ok(vec![])).is_err());
}

#[test]
fn modes_are_validated_deduplicated_and_prefer_bounded_high_resolution() {
    let mode = |width, height, fps, fourcc| Spec {
        width,
        height,
        fps,
        fourcc,
    };
    let hd = mode(1920, 1080, 30, *b"MJPG");
    let low = Spec::default();
    let large = mode(3840, 2160, 30, *b"MJPG");
    let modes = normalize(vec![
        low,
        hd,
        hd,
        large,
        mode(1920, 1080, 30, *b"YUYV"),
        mode(1920, 1080, 60, *b"MJPG"),
        mode(0, 480, 30, *b"MJPG"),
        mode(640, 480, 30, *b"H264"),
        mode(641, 480, 30, *b"YUYV"),
        mode(10000, 10000, 30, *b"MJPG"),
    ]);
    assert_eq!(modes.len(), 5);
    assert_eq!(preferred(&modes), Some(Spec { fps: 60, ..hd }));
    assert_eq!(preferred(&[large]), Some(large));
    assert_eq!(preferred(&[]), None);
    assert!(hd.to_string().contains("1920×1080"));
    assert!(hd.to_string().contains("MJPG"));
}

#[test]
fn query_arguments_reject_empty_extra_and_invalid_values_before_io() {
    for args in [
        vec![],
        vec![""],
        vec!["list", "extra"],
        vec!["modes"],
        vec!["modes", ""],
        vec!["modes", "-1"],
        vec!["modes", "4294967296"],
        vec!["modes", "0", "extra"],
    ] {
        assert!(
            worker(&args.iter().map(OsString::from).collect::<Vec<_>>())
                .is_err(),
            "{args:?}",
        );
    }
}

#[test]
fn resolutions_choose_highest_fps_then_mjpeg_independent_of_driver_order() {
    let hd = Spec {
        width: 1920,
        height: 1080,
        fps: 60,
        ..Spec::default()
    };
    let low = Spec {
        fourcc: *b"NV12",
        ..Spec::default()
    };
    let mut modes = vec![
        Spec { fps: 5, ..hd },
        Spec {
            fourcc: *b"YUYV",
            ..hd
        },
        Spec {
            fourcc: *b"NV12",
            ..hd
        },
        hd,
        hd,
        low,
        Spec {
            fps: 15,
            ..Spec::default()
        },
        Spec { width: 0, ..hd },
        Spec { fps: 0, ..hd },
        Spec {
            fourcc: *b"H264",
            ..hd
        },
    ];
    assert_eq!(resolutions(modes.clone()), vec![hd, low]);
    modes.reverse();
    assert_eq!(resolutions(modes), vec![hd, low]);
    assert_eq!(resolutions(vec![]), [] as [Spec; 0]);
    assert_eq!(resolutions(vec![Spec { fps: 0, ..hd }]), [] as [Spec; 0]);
    assert!(hd.to_string().contains("60 FPS · MJPG"));
}
