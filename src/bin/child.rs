use raw_sync::events::EventInit;
use shared_memory::ShmemConf;

fn main() {
    println!("Hello, child!");

    // open shared memory
    let send_shmem = ShmemConf::new().size(4096).flink("r_event_mapping").open().unwrap();
    let recv_shmem = ShmemConf::new().size(4096).flink("s_event_mapping").open().unwrap();


    // create event in shared memory
    let (send_event, _) = unsafe { raw_sync::events::Event::from_existing(send_shmem.as_ptr()).unwrap() };
    let (recv_event, _) = unsafe { raw_sync::events::Event::from_existing(recv_shmem.as_ptr()).unwrap() };

    let mut times = Vec::new();

    // sleep for 100ms to allow the parent to set up
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // acknowledge component init
    send_event.set(raw_sync::events::EventState::Signaled).unwrap();

    println!("Child ready to receive");
    
    let mut i = 0;
    loop {
        // wait for the parent to signal
        recv_event.wait(raw_sync::Timeout::Infinite).unwrap();

        i+=1;

        let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
        times.push(timestamp);

        // signal the parent
        send_event.set(raw_sync::events::EventState::Signaled).unwrap();

        // finish after 10,000 iterations
        if i == 50000 {
            break;
        }

    }

    println!("Goodbye, child! (Write)");

    let mut writer = csv::Writer::from_path("times-child.csv").unwrap();
    for (i, timestamp) in times.iter().enumerate() {
        writer
            .serialize((i, timestamp))
            .expect("Failed to write to file");
    }

    println!("Goodbye, child! (Done)");
}
