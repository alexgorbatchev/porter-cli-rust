#![cfg(target_os = "linux")]
use cli_command_kit::Mode;
use porter_cli::{
    command, entry, help_options,
    process::{Scanner, display_command, within_root},
    ui::{Action, Dashboard, action},
};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::Path,
};

#[test]
fn root_matching_obeys_path_components_and_command_arguments() {
    let root = Path::new("/work/repo");
    for candidate in ["/work/repo", "/work/repo/apps/web", "/work/repo/..foo"] {
        assert!(within_root(root, Path::new(candidate)), "{candidate}");
    }
    for candidate in [
        "/work/repox",
        "/work",
        "/elsewhere",
        "/work/repo/../outside",
    ] {
        assert!(!within_root(root, Path::new(candidate)), "{candidate}");
    }
    let shown = display_command(
        root,
        Path::new("/work/repo/apps/web"),
        b"/work/repo/node_modules/.bin/bun\0dev\0\0a b\0",
        None,
    );
    assert_eq!(shown, "apps/web $ node_modules/.bin/bun dev \"\" \"a b\"");
    assert_eq!(
        display_command(root, root, b"/usr/bin/server\0/work/repo/file\0", None),
        "server file"
    );
    assert_eq!(
        display_command(root, root, b"", Some(Path::new("/usr/bin/server"))),
        "server"
    );
    assert_eq!(display_command(root, root, b"", None), "<unknown>");
    assert_eq!(
        display_command(root, root, b"relative\0last", None),
        "relative last"
    );
    assert!(display_command(root, root, b"a\x1b[2J\0line\nnext\0", None).contains("\\u001b"));
    let home = std::env::home_dir().unwrap();
    let value = format!("/usr/bin/server\0{}\0", home.join("outside").display());
    assert_eq!(
        display_command(root, root, value.as_bytes(), None),
        "server ~/outside"
    );
}

#[test]
fn native_sock_diag_finds_current_ipv4_and_ipv6_listeners() {
    let ipv4 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let root = std::env::current_dir().unwrap();
    let rows = Scanner::new(root.clone()).scan().unwrap();
    assert!(
        rows.iter().any(|row| row.pid == std::process::id() as i32
            && row.port == ipv4.local_addr().unwrap().port())
    );
    if let Ok(ipv6) = std::net::TcpListener::bind("[::1]:0") {
        let rows = Scanner::new(root).scan().unwrap();
        assert!(rows.iter().any(|row| row.pid == std::process::id() as i32
            && row.port == ipv6.local_addr().unwrap().port()));
    }
    assert!(
        Scanner::new(Path::new("/path-that-does-not-match").into())
            .scan()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn process_handles_signal_only_the_disposable_selected_child() {
    let directory = tempfile::tempdir_in(".tmp").unwrap();
    let directory = directory.path().canonicalize().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "listener_child", "--nocapture"])
        .env("PORTER_TEST_CHILD", "1")
        .current_dir(&directory)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    while !line.contains("READY") {
        line.clear();
        assert!(reader.read_line(&mut line).unwrap() > 0);
    }
    let rows = Scanner::new(directory).scan().unwrap();
    let row = rows
        .iter()
        .find(|row| row.pid == child.id() as i32)
        .unwrap();
    row.terminate().unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(row.terminate().is_err());
}

#[test]
#[ignore = "subprocess fixture"]
fn listener_child() {
    if std::env::var("PORTER_TEST_CHILD").as_deref() != Ok("1") {
        return;
    }
    let _listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    println!("READY");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn dashboard_renders_status_selection_and_tiny_screens() {
    use ratatui::{Terminal, backend::TestBackend};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let root = std::env::current_dir().unwrap();
    let rows = Scanner::new(root.clone()).scan().unwrap();
    assert!(!rows.is_empty());
    let mut state = Dashboard::default();
    state.replace(Ok(rows));
    state.apply(Action::Down);
    state.apply(Action::Up);
    state.apply(Action::Up);
    for (width, height) in [(100, 10), (15, 3), (1, 1), (0, 0)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| state.draw(frame, &root, Mode::Human))
            .unwrap();
        assert_eq!(terminal.backend().buffer().area.width, width);
    }
    let mut terminal = Terminal::new(TestBackend::new(100, 10)).unwrap();
    state.replace(Err(anyhow::anyhow!("fixture scanner failure")));
    terminal
        .draw(|frame| state.draw(frame, &root, Mode::Agent))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("fixture scanner failure"), "{text}");
    let mut empty = Dashboard::default();
    empty.replace(Ok(Vec::new()));
    empty.apply(Action::Down);
    empty.apply(Action::Up);
    empty.apply(Action::Terminate);
    terminal
        .draw(|frame| empty.draw(frame, &root, Mode::Human))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("no process selected"), "{text}");
    drop(listener);
}

#[test]
fn native_key_events_match_controls() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    for (key, expected) in [
        (KeyCode::Char('j'), Action::Down),
        (KeyCode::Down, Action::Down),
        (KeyCode::Char('k'), Action::Up),
        (KeyCode::Up, Action::Up),
        (KeyCode::Char('x'), Action::Terminate),
        (KeyCode::Char('r'), Action::Refresh),
        (KeyCode::Char('q'), Action::Quit),
        (KeyCode::Char('z'), Action::None),
    ] {
        assert_eq!(
            action(Event::Key(KeyEvent::new(key, KeyModifiers::NONE))),
            expected
        );
    }
    assert_eq!(
        action(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        ))),
        Action::Quit
    );
    assert_eq!(action(Event::Resize(10, 2)), Action::None);
    let mut key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;
    assert_eq!(action(Event::Key(key)), Action::None);
}

fn invoke(args: &[&str], tty: bool, mode: Mode) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = entry(
        args.iter().map(OsString::from),
        tty,
        &mut out,
        &mut err,
        mode,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn metadata_and_terminal_validation_work_in_both_modes() {
    for mode in [Mode::Human, Mode::Agent] {
        assert_eq!(
            invoke(&["porter", "--version"], false, mode),
            (0, "0.1.0\n".into(), String::new())
        );
        assert_eq!(
            invoke(&["porter", "skill"], false, mode).1,
            help_options().skill.unwrap()
        );
        assert_eq!(invoke(&["porter"], false, mode).0, 1);
        for args in [
            vec!["porter", "--help"],
            vec!["porter", "skill", "--help"],
            vec!["porter", "help"],
            vec!["porter", "help", "skill"],
            vec!["porter", "help", "help"],
        ] {
            let result = invoke(&args, false, mode);
            assert_eq!(result.0, 0);
            assert_eq!(result.1.starts_with("ALERT:"), mode == Mode::Agent);
        }
        for args in [
            vec!["porter", "file"],
            vec!["porter", "--bad"],
            vec!["porter", "skill", "extra"],
        ] {
            assert_eq!(invoke(&args, false, mode).0, 1);
        }
    }
    let doc = std::fs::read_to_string("skills/porter/SKILL.md").unwrap();
    assert!(doc.contains("name: porter"));
    assert!(
        cli_command_kit::skill_omissions(&command(), &help_options(), &doc).is_empty(),
        "{:?}",
        cli_command_kit::skill_omissions(&command(), &help_options(), &doc)
    );
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("writer failed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn metadata_propagates_output_failures_and_native_binary_reports_once() {
    for args in [
        vec!["porter", "--help"],
        vec!["porter", "--version"],
        vec!["porter", "skill"],
    ] {
        let mut err = Vec::new();
        assert_eq!(
            entry(
                args.iter().map(OsString::from),
                false,
                &mut Broken,
                &mut err,
                Mode::Human
            ),
            1
        );
        assert!(String::from_utf8(err).unwrap().contains("writer failed"));
    }
    assert_eq!(
        entry(
            [OsString::from("porter"), OsString::from("--bad")],
            false,
            &mut Vec::new(),
            &mut Broken,
            Mode::Human
        ),
        1
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_porter"))
        .env("AGENT", "1")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(String::from_utf8(result.stderr).unwrap().lines().count(), 1);
}

#[test]
fn native_pty_refresh_resize_signal_and_quit_restore_terminal() {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::{
        io::{BufRead, Read},
        process::{Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir_in(".tmp").unwrap();
    let directory = directory.path().canonicalize().unwrap();
    let mut fixture = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "listener_child", "--nocapture"])
        .env("PORTER_TEST_CHILD", "1")
        .current_dir(&directory)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = std::io::BufReader::new(fixture.stdout.take().unwrap());
    let mut line = String::new();
    while !line.contains("READY") {
        line.clear();
        assert!(reader.read_line(&mut line).unwrap() > 0);
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 10,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_porter"));
    command.cwd(&directory);
    command.env("TERM", "xterm-256color");
    command.env("AGENT", "1");
    let mut child = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);
    let mut writer = pair.master.take_writer().unwrap();
    let mut output = pair.master.try_clone_reader().unwrap();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = output.read(&mut buffer) {
            if count == 0 || tx.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut captured = Vec::new();
    let wait_for = |needle: &str, captured: &mut Vec<u8>| {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut parser = vt100::Parser::new(10, 100, 0);
        parser.process(captured);
        while !parser.screen().contents().contains(needle) {
            let bytes = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| {
                    panic!("wait {needle}: {error}; {}", parser.screen().contents())
                });
            parser.process(&bytes);
            captured.extend(bytes);
        }
    };
    wait_for("Tracking 1 listening process entries", &mut captured);
    writer.write_all(b"jkr").unwrap();
    writer.flush().unwrap();
    pair.master
        .resize(PtySize {
            rows: 3,
            cols: 15,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    pair.master
        .resize(PtySize {
            rows: 10,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    writer.write_all(b"x").unwrap();
    writer.flush().unwrap();
    wait_for("Sent SIGTERM", &mut captured);
    assert!(!fixture.wait().unwrap().success());
    wait_for("No matching listening TCP ports under cwd", &mut captured);
    // Keep the session alive through the regular 750ms refresh too.
    captured.clear();
    let refresh_deadline = Instant::now() + Duration::from_millis(900);
    while Instant::now() < refresh_deadline {
        match rx.recv_timeout(refresh_deadline.saturating_duration_since(Instant::now())) {
            Ok(bytes) => captured.extend(bytes),
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("porter exited during refresh"),
        }
    }
    assert!(child.try_wait().unwrap().is_none());
    writer.write_all(b"q").unwrap();
    writer.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "porter did not quit");
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => captured.extend(bytes),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
    }
    drop(writer);
    drop(pair.master);
    worker.join().unwrap();
    while let Ok(bytes) = rx.try_recv() {
        captured.extend(bytes);
    }
    assert!(
        captured
            .windows(b"\x1b[?1049l".len())
            .any(|bytes| bytes == b"\x1b[?1049l"),
        "alternate screen was not restored: {:?}",
        String::from_utf8_lossy(&captured)
    );
}

#[test]
fn native_netlink_decoding_rejects_partial_and_failed_dumps() {
    use netlink_packet_core::{
        DoneMessage, ErrorMessage, NLM_F_DUMP_INTR, NetlinkHeader, NetlinkMessage, NetlinkPayload,
    };
    use netlink_packet_sock_diag::{
        SockDiagMessage,
        constants::{AF_INET, TCP_LISTEN},
        inet::{InetResponse, InetResponseHeader, SocketId},
    };
    use porter_cli::socket_diag::{Listeners, apply_datagram};
    fn serialize(payload: NetlinkPayload<SockDiagMessage>, sequence: u32, flags: u16) -> Vec<u8> {
        let mut header = NetlinkHeader::default();
        header.sequence_number = sequence;
        header.flags = flags;
        let mut packet = NetlinkMessage::new(header, payload);
        packet.finalize();
        let mut bytes = vec![0; packet.buffer_len()];
        packet.serialize(&mut bytes);
        bytes
    }
    let mut listeners = Listeners::new();
    let response = |state| {
        let mut id = SocketId::new_v4();
        id.source_port = 3000;
        SockDiagMessage::InetResponse(Box::new(InetResponse {
            header: InetResponseHeader {
                family: AF_INET,
                state,
                timer: None,
                socket_id: id,
                recv_queue: 0,
                send_queue: 0,
                uid: 1000,
                inode: 123,
            },
            nlas: Default::default(),
        }))
    };
    for state in [TCP_LISTEN, TCP_LISTEN, 1] {
        assert!(
            !apply_datagram(&serialize(response(state).into(), 3, 0), 3, &mut listeners).unwrap()
        );
    }
    assert_eq!(listeners[&123].iter().copied().collect::<Vec<_>>(), [3000]);
    let mut done = DoneMessage::default();
    done.code = 0;
    assert!(
        !apply_datagram(
            &serialize(NetlinkPayload::Done(done.clone()), 4, 0),
            3,
            &mut listeners
        )
        .unwrap()
    );
    assert!(
        apply_datagram(
            &serialize(NetlinkPayload::Done(done.clone()), 3, 0),
            3,
            &mut listeners
        )
        .unwrap()
    );
    assert!(
        apply_datagram(
            &serialize(NetlinkPayload::Done(done.clone()), 3, NLM_F_DUMP_INTR),
            3,
            &mut listeners
        )
        .is_err()
    );
    done.code = -5;
    assert!(
        apply_datagram(
            &serialize(NetlinkPayload::Done(done), 3, 0),
            3,
            &mut listeners
        )
        .is_err()
    );
    let mut error = ErrorMessage::default();
    error.code = std::num::NonZeroI32::new(-13);
    assert!(
        apply_datagram(
            &serialize(NetlinkPayload::Error(error), 3, 0),
            3,
            &mut listeners
        )
        .is_err()
    );
    assert!(
        !apply_datagram(
            &serialize(NetlinkPayload::Error(ErrorMessage::default()), 3, 0),
            3,
            &mut listeners
        )
        .unwrap()
    );
    assert!(
        apply_datagram(
            &serialize(NetlinkPayload::Overrun(vec![]), 3, 0),
            3,
            &mut listeners
        )
        .is_err()
    );
    assert!(apply_datagram(&[0; 3], 3, &mut listeners).is_err());
    let mut bytes = serialize(response(TCP_LISTEN).into(), 3, 0);
    bytes.truncate(bytes.len() - 1);
    assert!(apply_datagram(&bytes, 3, &mut listeners).is_err());
}
