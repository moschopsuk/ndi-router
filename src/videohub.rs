use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct VideoHub {
    input_lables: Vec<String>,
    output_lables: Vec<String>,
    routes: HashMap<u8, u8>,
    locks: HashMap<u8, Option<SocketAddr>>,
}

impl VideoHub {
    pub fn new(num_inputs: usize, num_outputs: usize) -> VideoHub {
        let mut intial_routing: HashMap<u8, u8> = HashMap::with_capacity(num_outputs);
        let mut intial_output_labels = Vec::with_capacity(num_outputs);
        let mut intial_input_labels = Vec::with_capacity(num_inputs);
        let mut intial_locks = HashMap::with_capacity(num_outputs);

        for x in 0..num_inputs {
            intial_input_labels.push(format!("NDI Input {}", x));
        }

        for x in 0..num_outputs {
            intial_routing.insert(x as u8, x as u8);
            intial_output_labels.push(format!("NDI Output {}", x));
            intial_locks.insert(x as u8, None);
        }

        VideoHub {
            input_lables: intial_input_labels,
            output_lables: intial_output_labels,
            routes: intial_routing,
            locks: intial_locks,
        }
    }

    pub fn preamble(self) -> String {
        format!("PROTOCOL PREAMBLE:\nVersion: {}\n\n", 2.7)
    }

    pub fn device_info(self) -> String {
        let mut device_info: Vec<String> = Vec::new();

        device_info.push(format!("VIDEOHUB DEVICE:"));
        device_info.push(format!("Device present: true"));
        device_info.push(format!("Model name: Blackmagic Smart Videohub"));
        device_info.push(format!("Video inputs: {}", self.input_lables.len()));
        device_info.push(format!("Video processing units: 0"));
        device_info.push(format!("Video outputs: {}", self.output_lables.len()));
        device_info.push(format!("Video monitoring outputs: 0"));
        device_info.push(format!("Serial ports: 0"));
        device_info.push(format!("\n"));

        device_info.join("\n")
    }

    pub fn list_inputs(&mut self) -> String {
        let mut labels: Vec<String> = Vec::new();
        labels.push(format!("INPUT LABELS:"));

        for (i, label) in self.input_lables.iter().enumerate() {
            labels.push(format!("{} {}", i, label));
        }

        labels.push(format!("\n"));
        labels.join("\n")
    }

    pub fn list_outputs(self) -> String {
        let mut labels: Vec<String> = Vec::new();
        labels.push(format!("OUTPUT LABELS:"));

        for (i, label) in self.output_lables.iter().enumerate() {
            labels.push(format!("{} {}", i, label));
        }

        labels.push(format!("\n"));
        labels.join("\n")
    }

    pub fn list_routes(self) -> String {
        let mut labels: Vec<String> = Vec::new();
        labels.push(format!("VIDEO OUTPUT ROUTING:"));

        for (input, output) in self.routes.iter() {
            labels.push(format!("{} {}", input, output));
        }

        labels.push(format!("\n"));
        labels.join("\n")
    }

    pub fn list_locks(self) -> String {
        let mut labels: Vec<String> = Vec::new();
        labels.push(format!("VIDEO OUTPUT LOCKS:"));

        for (i, _) in self.locks.iter() {
            // let state = if *lock { "L" } else { "U" };
            labels.push(format!("{} {}", i, "U"));
        }

        labels.push(format!("\n"));
        labels.join("\n")
    }

    pub fn set_input_label(&mut self, index: usize, label: String) {
        std::mem::replace(&mut self.input_lables[index], label);
    }

    pub fn inital_status_dump(self) -> String {
        let mut initial_dump = Vec::new();

        initial_dump.push(self.clone().preamble());
        initial_dump.push(self.clone().device_info());
        initial_dump.push(self.clone().list_inputs());
        initial_dump.push(self.clone().list_outputs());
        initial_dump.push(self.clone().list_routes());
        initial_dump.push(self.clone().list_locks());
    
        initial_dump.join("")
    }
}
