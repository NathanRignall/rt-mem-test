use std::process::{Command, Stdio};

use libc::cpu_set_t;
use raw_sync::events::EventInit;
use shared_memory::ShmemConf;

fn main() {
    println!("Hello, parent!");

    // use libc to set the process core affinity to core 2
    let mut cpu_set: cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_SET(2, &mut cpu_set);
        let ret = libc::sched_setaffinity(0, std::mem::size_of_val(&cpu_set), &cpu_set);
        if ret != 0 {
            panic!("Failed to set affinity");
        }
    }

    // use libc to set the process sechdeuler to SCHEDULER FFIO
    unsafe {
        let ret = libc::sched_setscheduler(
            0,
            libc::SCHED_FIFO,
            &libc::sched_param { sched_priority: 99 },
        );
        if ret != 0 {
            println!("Failed to set scheduler");
        }
    }

    // open shared memory
    let send_shmem = ShmemConf::new()
        .size(4096)
        .flink("s_event_mapping")
        .create()
        .unwrap();
    let recv_shmem = ShmemConf::new()
        .size(4096)
        .flink("r_event_mapping")
        .create()
        .unwrap();

    // create event in shared memory
    let (send_event, _) =
        unsafe { raw_sync::events::Event::new(send_shmem.as_ptr(), true).unwrap() };
    let (recv_event, _) =
        unsafe { raw_sync::events::Event::new(recv_shmem.as_ptr(), true).unwrap() };

    // spawn the child process
    let binary_path = format!("target/release/child");
    let mut command = Command::new(binary_path);

    // redirect the child's stderr to the parent's stderr
    let child = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    // set the core affinity for the child process to core 3
    let mut cpu_set: cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_SET(3, &mut cpu_set);
        let ret = libc::sched_setaffinity(
            child.id() as libc::pid_t,
            std::mem::size_of_val(&cpu_set),
            &cpu_set,
        );
        if ret != 0 {
            panic!("Failed to set affinity");
        }
    }

    // set the scheduler for the child process
    unsafe {
        let ret = libc::sched_setscheduler(
            child.id() as libc::pid_t,
            libc::SCHED_FIFO,
            &libc::sched_param { sched_priority: 99 },
        );
        if ret != 0 {
            println!("Failed to set scheduler");
        }
    }

    // wait for the component to be ready
    recv_event.wait(raw_sync::Timeout::Infinite).unwrap();

    println!("Child is ready");

    // create vector to store timestamps
    let mut times = Vec::new();

    // store loop timing information
    let mut last_time;
    let mut last_sleep = std::time::Duration::from_micros(0);
    let mut last_duration = std::time::Duration::from_micros(0);
    let mut overruns = 0;
    let period = std::time::Duration::from_micros(1_000_000 / 100 as u64);

    println!("Parent ready to send");

    // now start looping to test the response time
    let mut i = 0;
    loop {
        last_time = std::time::Instant::now();

        // store the timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        times.push((
            timestamp,
            last_sleep.as_micros() as u64,
            last_duration.as_micros() as u64,
            overruns,
        ));

        i += 1;

        // signal the child
        send_event
            .set(raw_sync::events::EventState::Signaled)
            .unwrap();

        // wait for the child to signal back
        recv_event.wait(raw_sync::Timeout::Infinite).unwrap();

        // finish after 50,000 iterations
        if i == 50000 {
            break;
        }

        // update loop timing information
        let now = std::time::Instant::now();
        let duration = now.duration_since(last_time);
        let mut sleep = std::time::Duration::from_micros(0);

        if duration <= period {
            sleep = period - duration;
            std::thread::sleep(sleep);
        } else {
            overruns += 1;
            println!(
                "Warning: loop took longer than period {}us - {}us",
                duration.as_micros(),
                last_sleep.as_micros()
            );
        }

        last_duration = duration;
        last_sleep = sleep;
    }

    println!("Goodbye, parent! (Write)");

    // write the timestamps to a CSV file
    let mut writer = csv::Writer::from_path("times-parent.csv").unwrap();
    for (i, (timestamp, sleep, duration, overruns)) in times.iter().enumerate() {
        writer
            .serialize((i, timestamp, sleep, duration, overruns))
            .expect("Failed to write to file");
    }

    println!("Goodbye, parent! (Done)");
}
