mod package;

use sha2::{Sha256, Digest};
use package::{Package, VersionData};
use reqwest::get;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufReader, BufWriter, Write};
use std::{fs::File, u64};
use serde_json::{to_string_pretty, from_str, Value, to_writer_pretty};

#[tokio::main]
async fn main() {
    let data = get_packages().await;

    let val = data
        .as_str()
        .parse::<Value>()
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));

    let package_list: Vec<&str> = val["packages"]
        .as_array()
        .unwrap_or_else(|| handle_error_and_exit("An error occured".to_string()))
        .iter()
        .map(|p| {
            p.as_str()
                .unwrap_or_else(|| handle_error_and_exit("An error occured".to_string()))
        })
        .collect();
    
    for _package in package_list {
        // autoupdate("7-zip").await;
    }
    autoupdate("brave").await;
}

async fn autoupdate(package_name: &str) {
    let package: Package = get_package(package_name.clone()).await;
    let mut temp_package: Package = package.clone();
    let url = package.autoupdate.download_page;
    println!("url: {}", url);
    let response = get(url).await.unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response.text().await.unwrap_or_else(|e| handle_error_and_exit(e.to_string()));

    // println!("cont: {}", file_contents);

    let regex = regex::Regex::new(package.autoupdate.regex.as_str()).unwrap();

    let matches: Vec<&str> = regex.captures_iter(file_contents.as_str()).map(|c| c.get(0).unwrap().as_str()).collect();
    println!("matches: {:?}", matches);

    let mut versions_calc: Vec<String> = vec![];

    let mut versions: Vec<&str> = vec![];

    for mut _match in matches {
        let version_split: Vec<&str> = _match.split(" ").collect();
        _match = version_split[1].trim();
        if _match.contains("v") {
            let version_split: Vec<&str> = _match.split("v").collect();
            _match = version_split[1];
        }

        versions.push(_match);
        
        let year_dot_split: Vec<&str> = _match.split(".").collect();
        let year_string = year_dot_split.concat();
        versions_calc.push(year_string);
    }

    println!("version final: {:?}", versions);

    let index = versions_calc.iter()
    .enumerate()
    .filter_map(|(i, s)| s.parse::<u64>().ok().map(|n| (i, n)))
    .max_by_key(|&(_, n)| n)
    .map(|(i, _)| i).unwrap_or_else(|| handle_error_and_exit("Failed to find match".to_string()));    

    let version = versions[index];

    println!("latest version: {}", version);

    if &package.latest_version != version {
        let url = package.autoupdate.download_url.replace("<version>", version);
        println!("url: {}", url);
        let response = get(url.clone()).await.unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
        let file_size = response.content_length().unwrap_or_else(|| handle_error_and_exit("Failed to get content length".to_string()));

        let temp = std::env::var("TEMP").unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
        let loc = format!(r"{}\novus\{}_check.exe", temp, package_name);
        threadeddownload(url.clone(), loc.clone(), package.threads, package_name.to_string(), "".to_string(), false, false).await;
        let hash = get_checksum(loc.clone());

        let _ = std::fs::remove_file(loc);

        let version_data: VersionData = VersionData {
            url: url,
            size: file_size,
            checksum: hash,
        };

        println!("version_data: {:?}", version_data);

        // make changes to data
        temp_package.versions.insert(version.clone().to_string(), version_data);
        temp_package.latest_version = version.to_string();

        // Re-open file to replace the contents:
        let file = std::fs::File::create(format!(r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json", package_name)).unwrap(); 
        to_writer_pretty(file, &temp_package).unwrap();
        let mut commit = format!("autoupdate: {}", package_name);
        commit = "\"".to_string() + commit.as_str() + "\"";
        std::process::Command::new("powershell").arg("novus_update").output().expect("Failed to update gcp bucket");
        // std::process::Command::new("gsutil").args(&["cp", "-r", "\"D:/prana/Programming/My Projects/novus-package-manager/novus-packages/packages/*\"", "gs://novus_bucket"]).output().expect("Failed to update bucket");
        let dir = std::path::Path::new(r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\");
        let _ = std::env::set_current_dir(dir);
        std::process::Command::new("powershell").args(&["deploy", commit.as_str(), "main"]).output().expect("Failed to deploy to github");
        // std::process::Command::new("git").args(&["add", "."]).output().expect("Failed to add");
        // std::process::Command::new("git").args(&["commit", "-m", commit.as_str()]).output().expect("Failed to commit");
        // std::process::Command::new("git").args(&["push"]).output().expect("Failed to push");
    }
}

fn handle_error_and_exit(e: String) -> ! {
    println!("{}{:?}", "error ".bright_red(), e);
    std::process::exit(0);
}

async fn get_packages() -> String {    
    let response = get(format!("https://storage.googleapis.com/novus_bucket/package-list.json?a={:?}", std::time::UNIX_EPOCH.elapsed().unwrap())).await.unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response.text().await.unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:36", e.to_string())));
    let content: Value = from_str(file_contents.as_str()).unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:53", e.to_string())));
    to_string_pretty(&content).unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:54", e.to_string())))
}


async fn get_package(package_name: &str) -> Package {
    println!("getting: https://storage.googleapis.com/novus_bucket/{}.json?a={:?}", package_name, std::time::UNIX_EPOCH.elapsed().unwrap());
    let response = get(format!("https://storage.googleapis.com/novus_bucket/{}.json?a={:?}", package_name, std::time::UNIX_EPOCH.elapsed().unwrap())).await.unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response.text().await.unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:36", e.to_string())));
    from_str::<Package>(&file_contents).unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:29", e.to_string())))
}

fn get_checksum(output: String) -> String {
    let mut file = std::fs::File::open(output.clone()).unwrap();
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).unwrap();
    format!("{:x}", hasher.finalize()).to_uppercase()
}

async fn threadeddownload(
    url: String,
    output: String,
    threads: u64,
    package_name: String,
    checksum: String,
    get_checksum: bool,
    max: bool,
) {
    // let start = Instant::now();
    let mut handles = vec![];
    let res = reqwest::get(url.to_string())
        .await
        .unwrap_or_else(|_| handle_error_and_exit("Failed to get download url!".to_string()));
    let total_length = res
        .content_length()
        .unwrap_or_else(|| handle_error_and_exit("An Unexpected Error Occured!".to_string()));
    let temp = std::env::var("TEMP").unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:106", e.to_string())));

    if max {
        let progress_bar = ProgressBar::new(total_length);
        progress_bar.set_style(ProgressStyle::default_bar()
              .template("[{elapsed_precise}] [{wide_bar:.cyan/blue/magenta}] {bytes}/{total_bytes} ({eta})")
              .progress_chars("=>-"));

        for index in 0..threads {
            let loc = format!(r"{}\novus\setup_{}{}.tmp", temp, package_name, index + 1);
            let (start, end) = get_splits(index + 1, total_length, threads);
            let pb = progress_bar.clone();
            let mut file = BufWriter::new(
                File::create(loc).unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:119", e.to_string()))),
            );
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:129", e.to_string())));

                while let Some(chunk) = response
                    .chunk()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:134", e.to_string())))
                {
                    pb.inc(chunk.len() as u64);
                    let _ = file.write(&*chunk);
                }
            }));
        }

        futures::future::join_all(handles).await;

        progress_bar.finish();
    } else {
        for index in 0..threads {
            let loc = format!(r"{}\novus\setup_{}{}.tmp", temp, package_name, index + 1);
            let (start, end) = get_splits(index + 1, total_length, threads);
            let mut file = BufWriter::new(
                File::create(loc).unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:150", e.to_string()))),
            );
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:160", e.to_string())));
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:164", e.to_string())))
                {
                    let _ = file.write(&*chunk);
                }
            }));
        }

        futures::future::join_all(handles).await;
    }

    let mut file =
        File::create(output.clone()).unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:175", e.to_string())));

    let temp = std::env::var("TEMP").unwrap();

    for index in 0..threads {
        let loc = format!(r"{}\novus\setup_{}{}.tmp", temp, package_name, index + 1);
        let mut buf: Vec<u8> = vec![];
        let downloaded_file =
            File::open(loc.clone()).unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:183", e.to_string())));
        let mut reader = BufReader::new(downloaded_file);
        let _ = std::io::copy(&mut reader, &mut buf);
        let _ = file.write_all(&buf);        
        let _ = std::fs::remove_file(loc);
    }    

    if get_checksum {
        verify_checksum(output, checksum);
    }

    // println!("download time: {:?}", start.elapsed());
}

fn verify_checksum(output: String, checksum: String) {
    let mut file = std::fs::File::open(output.clone()).unwrap();
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).unwrap();
    let hash = format!("{:x}", hasher.finalize());

    if hash.to_uppercase() == checksum.to_uppercase() {
        // Verified Checksum
        println!("{}", "Successfully Verified Hash".bright_green());
    } else {
        handle_error_and_exit("Failed To Verify Hash".to_string());
    }
}

fn get_splits(i: u64, total_length: u64, threads: u64) -> (u64, u64) {
    let mut start = ((total_length / threads) * (i - 1)) + 1;
    let mut end = (total_length / threads) * i;

    if i == 1 {
        start = 0;
    }

    if i == threads {
        end = total_length;
    }

    (start, end)
}