use std::path::Path;

use crate::core::auto_update::{
    AutoUpdateIntervalUnit, AutoUpdateSchedule, AutoUpdateScheduleType,
};
use crate::core::system_scheduler::{
    build_launch_agent_plist, build_systemd_service, build_systemd_timer, launchctl_kickstart_args,
    scheduler_executable_for_current_exe, summarize_launchctl_status, systemd_start_args,
    windows_schtasks_args, windows_schtasks_run_args, SchedulerConfig,
};

fn hourly_schedule(hours: i64) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Interval,
        interval_value: hours,
        interval_unit: AutoUpdateIntervalUnit::Hours,
        daily_time: "03:00".to_string(),
    }
}

fn minute_schedule(minutes: i64) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Interval,
        interval_value: minutes,
        interval_unit: AutoUpdateIntervalUnit::Minutes,
        daily_time: "03:00".to_string(),
    }
}

fn daily_schedule(time: &str) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Daily,
        interval_value: 24,
        interval_unit: AutoUpdateIntervalUnit::Hours,
        daily_time: time.to_string(),
    }
}

#[test]
fn mac_launch_agent_uses_current_user_interval_and_background_arg() {
    let plist = build_launch_agent_plist(&SchedulerConfig {
        executable: Path::new("/Applications/Skills Hub.app/Contents/MacOS/Skills Hub")
            .to_path_buf(),
        schedule: hourly_schedule(24),
    });

    assert!(plist.contains("<key>StartInterval</key>"));
    assert!(plist.contains("<integer>86400</integer>"));
    assert!(plist.contains("--background-task</string>"));
    assert!(plist.contains("update-skills</string>"));
    assert!(plist.contains("--force</string>"));
}

#[test]
fn mac_launch_agent_supports_daily_time() {
    let plist = build_launch_agent_plist(&SchedulerConfig {
        executable: Path::new("/Applications/Skills Hub.app/Contents/MacOS/Skills Hub")
            .to_path_buf(),
        schedule: daily_schedule("03:30"),
    });

    assert!(plist.contains("<key>StartCalendarInterval</key>"));
    assert!(plist.contains("<key>Hour</key>"));
    assert!(plist.contains("<integer>3</integer>"));
    assert!(plist.contains("<key>Minute</key>"));
    assert!(plist.contains("<integer>30</integer>"));
    assert!(!plist.contains("<key>StartInterval</key>"));
}

#[test]
fn debug_scheduler_uses_stable_runner_copy_for_target_debug_binary() {
    let temp = tempfile::tempdir().unwrap();
    let debug_dir = temp.path().join("target").join("debug");
    std::fs::create_dir_all(&debug_dir).unwrap();
    let app = debug_dir.join("app");
    std::fs::write(&app, b"runner").unwrap();

    let executable = scheduler_executable_for_current_exe(&app).unwrap();

    if cfg!(debug_assertions) {
        assert_eq!(executable, debug_dir.join("skills-hub-autoupdate-runner"));
        assert_eq!(std::fs::read(&executable).unwrap(), b"runner");
    } else {
        assert_eq!(executable, app);
    }
}

#[test]
fn mac_kickstart_targets_user_launch_agent() {
    let args = launchctl_kickstart_args(501);

    assert_eq!(
        args,
        vec!["kickstart", "-k", "gui/501/com.skillshub.autoupdate"]
    );
}

#[test]
fn mac_launchctl_status_summary_includes_runtime_state() {
    let summary = summarize_launchctl_status(
        r#"
        state = not running
        runs = 34
        last exit code = 0
        "#,
    );

    assert_eq!(summary, "state = not running; last exit code = 0");
}

#[test]
fn windows_task_uses_hourly_schedule_without_elevated_flag() {
    let args = windows_schtasks_args(&SchedulerConfig {
        executable: Path::new("C:\\Users\\may\\AppData\\Local\\SkillsHub\\SkillsHub.exe")
            .to_path_buf(),
        schedule: hourly_schedule(12),
    })
    .unwrap();

    assert!(args.iter().any(|v| v == "/SC"));
    assert!(args.iter().any(|v| v == "HOURLY"));
    assert!(args.iter().any(|v| v == "/MO"));
    assert!(args.iter().any(|v| v == "12"));
    assert!(!args.iter().any(|v| v == "/RL"));
    assert!(args
        .iter()
        .any(|v| v.contains("--background-task update-skills --force")));
}

#[test]
fn windows_task_supports_minutes_and_daily_time() {
    let minute_args = windows_schtasks_args(&SchedulerConfig {
        executable: Path::new("C:\\SkillsHub\\SkillsHub.exe").to_path_buf(),
        schedule: minute_schedule(30),
    })
    .unwrap();
    assert!(minute_args.iter().any(|v| v == "MINUTE"));
    assert!(minute_args.iter().any(|v| v == "30"));

    let daily_args = windows_schtasks_args(&SchedulerConfig {
        executable: Path::new("C:\\SkillsHub\\SkillsHub.exe").to_path_buf(),
        schedule: daily_schedule("23:45"),
    })
    .unwrap();
    assert!(daily_args.iter().any(|v| v == "DAILY"));
    assert!(daily_args.iter().any(|v| v == "/ST"));
    assert!(daily_args.iter().any(|v| v == "23:45"));
}

#[test]
fn windows_task_maps_whole_day_intervals_to_daily_schedule() {
    for (hours, days) in [(24, "1"), (48, "2"), (720, "30")] {
        let args = windows_schtasks_args(&SchedulerConfig {
            executable: Path::new("C:\\SkillsHub\\SkillsHub.exe").to_path_buf(),
            schedule: hourly_schedule(hours),
        })
        .unwrap();

        assert!(args.iter().any(|value| value == "DAILY"));
        assert!(args.iter().any(|value| value == "/MO"));
        assert!(args.iter().any(|value| value == days));
        assert!(!args.iter().any(|value| value == "HOURLY"));
    }
}

#[test]
fn windows_task_keeps_23_hours_as_hourly_schedule() {
    let args = windows_schtasks_args(&SchedulerConfig {
        executable: Path::new("C:\\SkillsHub\\SkillsHub.exe").to_path_buf(),
        schedule: hourly_schedule(23),
    })
    .unwrap();

    assert!(args.iter().any(|value| value == "HOURLY"));
    assert!(args.iter().any(|value| value == "23"));
}

#[test]
fn windows_task_rejects_long_non_whole_day_intervals() {
    let error = windows_schtasks_args(&SchedulerConfig {
        executable: Path::new("C:\\SkillsHub\\SkillsHub.exe").to_path_buf(),
        schedule: hourly_schedule(25),
    })
    .unwrap_err();

    assert!(error.to_string().contains("whole days"));
}

#[test]
fn windows_run_args_start_registered_task() {
    let args = windows_schtasks_run_args();

    assert_eq!(args, vec!["/Run", "/TN", "com.skillshub.autoupdate"]);
}

#[test]
fn linux_systemd_unit_uses_user_timer_interval() {
    let config = SchedulerConfig {
        executable: Path::new("/usr/bin/skills-hub").to_path_buf(),
        schedule: hourly_schedule(48),
    };

    let service = build_systemd_service(&config);
    let timer = build_systemd_timer(&config);

    assert!(
        service.contains("ExecStart=/usr/bin/skills-hub --background-task update-skills --force")
    );
    assert!(timer.contains("OnBootSec=5min"));
    assert!(timer.contains("OnUnitActiveSec=2880min"));
    assert!(timer.contains("WantedBy=timers.target"));
}

#[test]
fn linux_systemd_timer_supports_minutes_and_daily_time() {
    let minute_timer = build_systemd_timer(&SchedulerConfig {
        executable: Path::new("/usr/bin/skills-hub").to_path_buf(),
        schedule: minute_schedule(45),
    });
    assert!(minute_timer.contains("OnUnitActiveSec=45min"));

    let daily_timer = build_systemd_timer(&SchedulerConfig {
        executable: Path::new("/usr/bin/skills-hub").to_path_buf(),
        schedule: daily_schedule("04:15"),
    });
    assert!(daily_timer.contains("OnCalendar=*-*-* 04:15:00"));
}

#[test]
fn linux_start_args_start_service_for_immediate_test() {
    let args = systemd_start_args();

    assert_eq!(
        args,
        vec!["--user", "start", "com.skillshub.autoupdate.service"]
    );
}
