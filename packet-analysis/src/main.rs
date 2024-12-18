use ndarray::Array1;
use ndarray_npz::NpzWriter;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

// Preallocate memory for each IP to hold up to 4M entries
const PREALLOC_ENTRIES: usize = 4_000_000;

// Function to find files matching the pattern "packet-data/*_qos*"
fn find_matching_files(directory: &str, pattern: &str) -> Vec<String> {
    let mut matching_files = Vec::new();

    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(file_name) = path.file_name() {
                    if let Some(file_name_str) = file_name.to_str() {
                        if file_name_str.contains(pattern) {
                            if let Some(path_str) = path.to_str() {
                                matching_files
                                    .push(path_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    matching_files
}

fn ip_values(files: &[String]) -> HashMap<String, Vec<u64>> {
    let mut results: HashMap<String, Vec<u64>> = HashMap::new();

    for file in files {
        if let Ok(f) = File::open(file) {
            let reader = BufReader::new(f);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let mut parts = line.split_whitespace();
                    // Ensure the line has exactly two elements (number and IP)
                    if let (Some(number_str), Some(ip), None) =
                        (parts.next(), parts.next(), parts.next())
                    {
                        if let Ok(number) = number_str.parse::<u64>() {
                            let entry = results
                                .entry(ip.to_string())
                                .or_insert_with(|| {
                                    Vec::with_capacity(PREALLOC_ENTRIES)
                                });
                            entry.push(number);
                        }
                    }
                }
            }
        }
    }

    results
}

fn write_to_npz(results: HashMap<String, Vec<u64>>, output_path: &str) {
    // Create an .npz file
    let file = File::create(output_path)
        .expect("Failed to create output file");
    let mut npz = NpzWriter::new(file);

    for (ip, values) in results {
        // Convert the Vec<u64> to an ndarray
        let array = Array1::from(values);
        // Save each IP's values as a separate array entry in the .npz file
        npz.add_array(&ip, &array)
            .expect("Failed to write array");
    }
}

fn main() {
    for (pattern, name) in [("_qos", "input"), ("sigverify", "output")]
    {
        // Find files matching the pattern "packet-data/*_qos*"
        let files = find_matching_files("packet-data", pattern);

        // Process IP values from the matched files
        let result = ip_values(&files);

        // Write the results to an .npz file
        write_to_npz(result, &format!("{name}.npz"));
    }
}
