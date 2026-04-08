use daemonize::Daemonize;
use serde::{Deserialize, Serialize};
use std::{
    cmp::max,
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, RwLock},
    time::Duration,
};

fn main() -> anyhow::Result<()> {
    let home = std::env::home_dir().expect("user must have home");
    let slop_dir = home.join(".slop");

    // create slop dir
    if !std::fs::exists(&slop_dir)? {
        std::fs::create_dir(&slop_dir)?;
    }

    let slop_file_path = slop_dir.join("slop.toml");
    let config = if !std::fs::exists(slop_dir.join("slop.toml"))? {
        // create a new user flow
        println!("Enter your username to enable slopmon:");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).expect("invalid string");
        let config = Config {
            user: s.clone().trim().into(),
        };

        let mut slop_file = File::create(slop_file_path).expect("unable to create slop toml");
        let _ = slop_file.write(toml::to_string(&config)?.as_bytes())?;
        config
    } else {
        let slop_file = std::fs::read_to_string(&slop_file_path)?;
        let config = toml::from_str(&slop_file)?;
        config
    };
    let config = Arc::from(config);

    let stdout = File::create(slop_dir.join("daemon.out"))?;
    let stderr = File::create(slop_dir.join("daemon.err"))?;
    let pid_file = slop_dir.join("slop.pid");

    let daemonize = Daemonize::new()
        .pid_file(pid_file)
        .chown_pid_file(true)
        .working_directory(&slop_dir)
        .stderr(stderr)
        .stdout(stdout);

    match daemonize.start() {
        Ok(_) => run_process(&home.as_path(), &config)?,
        Err(e) => println!("error: {}", e),
    }

    Ok(())
}

fn run_process(home: &Path, config: &Arc<Config>) -> anyhow::Result<()> {
    let file_offset_map: Arc<RwLock<HashMap<String, u32>>> = Arc::new(RwLock::new(HashMap::new()));
    let config = Arc::clone(config);

    let mut debouncer = notify_debouncer_mini::new_debouncer(
        Duration::from_secs(5),
        move |res: notify_debouncer_mini::DebounceEventResult| match res {
            Ok(events) => {
                let file_offset_map = Arc::clone(&file_offset_map);
                for event in events {
                    if event.path.to_str().unwrap().contains("jsonl") {
                        println!("emitted: {:?}", event);
                        let start_offset = file_offset_map
                            .read()
                            .unwrap()
                            .get(event.path.to_str().unwrap())
                            .unwrap_or(&0)
                            .clone();

                        match process_codex_event(&event, &start_offset) {
                            Ok(output) => {
                                println!("output: {:?}", output);
                                // update file offset map with newest recorded offset
                                file_offset_map
                                    .write()
                                    .unwrap()
                                    .insert(output.path, output.new_offset);

                                if output.new_max_tokens > 0 {
                                    // todo: send an api request or something here with the new max token count for the user
                                    let user = config.user.clone();
                                    let tokens = output.new_max_tokens;
                                    let req = LeaderboardPayload { user, tokens };

                                    println!("request payload: {:#?}", req);
                                }
                            }
                            Err(e) => println!("error: {:#?}", e),
                        }
                    }
                }
            }
            Err(e) => println!("error: {:#?}", e),
        },
    )?;

    debouncer.watcher().watch(
        home.join(".codex/sessions").as_path(),
        notify::RecursiveMode::Recursive,
    )?;

    loop {}

    Ok(())
}

fn process_codex_event(
    event: &notify_debouncer_mini::DebouncedEvent,
    start_offset: &u32,
) -> anyhow::Result<ProcessingOutput> {
    let binding = std::fs::read_to_string(event.path.clone())?;
    let mut file_lines = binding.lines();
    for _ in 0..*start_offset {
        file_lines.next();
    }

    let mut offset_delta = 0;
    let mut max_tokens = 0;
    for line in file_lines {
        offset_delta += 1;
        if !line.contains("total_token_usage") {
            continue;
        }
        println!("{}", line);

        let message: Message = serde_json::from_str(line)?;
        if message.payload.info.total_token_usage.total_tokens > max_tokens {
            max_tokens = message.payload.info.total_token_usage.total_tokens;
        }
        println!("max tokens: {}", max_tokens)
    }

    let new_offset = start_offset + offset_delta;

    Ok(ProcessingOutput {
        path: event.path.to_str().unwrap().into(),
        new_max_tokens: max_tokens,
        new_offset,
    })
}

#[derive(Debug)]
struct ProcessingOutput {
    path: String,
    new_max_tokens: u32,
    new_offset: u32,
}

#[derive(Serialize, Deserialize)]
struct Config {
    user: String,
}

#[derive(Deserialize)]
struct Message {
    payload: MessagePayload,
}

#[derive(Deserialize)]
struct MessagePayload {
    info: MessageInfo,
}

#[derive(Deserialize)]
struct MessageInfo {
    total_token_usage: MessageTokenUsage,
}

#[derive(Deserialize)]
struct MessageTokenUsage {
    total_tokens: u32,
}

#[derive(Serialize, Debug)]
struct LeaderboardPayload {
    tokens: u32,
    user: String,
}
