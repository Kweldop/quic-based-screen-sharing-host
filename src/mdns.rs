use std::collections::HashMap;

use mdns_sd::{DaemonEvent, IfKind, Receiver, ServiceDaemon, ServiceInfo};

use crate::error::AppError;

pub struct Mdns {
    mdns: mdns_sd::ServiceDaemon,
    properties: HashMap<String, String>,
    service_type: String,
    hostname: String,
    port: u16,
    fullname: Option<String>,
}

impl Mdns {
    pub fn new() -> Result<Self, AppError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;
        let service_type = "_remote-desktop._tcp.local.";
        let hostname = sysinfo::System::host_name()
            .ok_or(AppError::XCustomMessage("Couldn't find host name"))?;
        let port = 3456;
        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("quic_port".to_string(), "1700".to_string());
        Ok(Self {
            mdns,
            properties: properties,
            service_type: service_type.to_string(),
            hostname,
            port,
            fullname: None,
        })
    }

    pub fn add_prop(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }

    pub fn start_service(&mut self) -> Result<Receiver<DaemonEvent>, AppError> {
        let my_addrs = "";
        let service_info = ServiceInfo::new(
            &self.service_type,
            &self.hostname,
            &format!("{}.local.", self.hostname),
            my_addrs,
            self.port,
            self.properties.clone(),
        )?
        .enable_addr_auto();

        self.fullname = Some(service_info.get_fullname().to_string());
        self.mdns.register(service_info)?;
        Ok(self.mdns.monitor()?)
    }

    pub fn stop_service(&self) -> Result<(), AppError> {
        let fullname = self
            .fullname
            .clone()
            .ok_or(AppError::XCustomMessage("Service info not found"))?;

        self.mdns.unregister(&fullname)?;
        Ok(())
    }

    pub fn monitor_daemon(event: Receiver<DaemonEvent>) -> Result<(), AppError> {
        loop {
            match event.recv() {
                Ok(event) => {
                    println!("Daemon event: {:?}", &event);
                    if let DaemonEvent::Error(e) = event {
                        println!("Failed: {}", e);
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        Ok(())
    }
}
