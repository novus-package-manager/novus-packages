mod package;

use clipboard_win::{formats, set_clipboard};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use package::{AutoUpdateData, Package, VersionData};
use reqwest::get;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string_pretty, to_writer_pretty, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::{fs::File, u64};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("args: {:?}", args);
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

    std::process::Command::new("powershell")
        .arg("novus_update")
        .output()
        .expect("Failed to update gcp bucket");

    if args.len() == 1 {
        for package in package_list {
            autoupdate(package).await;
        }
    } else {
        if args[1] == "new" {
            new_package(&args[2]);
        } else if args[1] == "test" {
            get_contents(&args[2]).await;
        } else if args[1] == "remove" {
            remove(&args[2]);
        } else {
            autoupdate(&args[1]).await;
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PackageList {
    packages: Vec<String>,
}

async fn get_contents(package_name: &str) {
    let package: Package = get_package(package_name.clone()).await;
    let url = package.clone().autoupdate.download_page;
    println!("url: {}", url);
    let response = get(url)
        .await
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));

    println!("cont: {}", file_contents);

    set_clipboard(formats::Unicode, file_contents).expect("To set clipboard");
}

fn new_package(package_name: &String) {
    let loc = format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json",
        package_name
    );
    let path = std::path::Path::new(&loc);
    let package_list_loc = std::path::Path::new(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\package-list\package-list.json",
    );
    let file_contents = std::fs::read_to_string(package_list_loc).unwrap();
    let package_list: PackageList =
        serde_json::from_str::<PackageList>(file_contents.as_str()).unwrap();
    let mut packages: Vec<String> = package_list.packages;
    packages.push(package_name.clone());
    packages.sort();
    let package_list: PackageList = PackageList { packages: packages };
    let file = std::fs::File::create(r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\package-list\package-list.json").unwrap();
    to_writer_pretty(file, &package_list).unwrap();
    let package_file = std::fs::File::create(path).unwrap();
    let package: Package = Package {
        package_name: package_name.clone(),
        display_name: String::new(),
        latest_version: "0".to_string(),
        threads: 8,
        iswitches: vec![],
        uswitches: vec![],
        autoupdate: AutoUpdateData {
            download_page: String::new(),
            download_url: String::new(),
            regex: String::new(),
        },
        versions: HashMap::new(),
    };
    to_writer_pretty(package_file, &package).unwrap();
}

async fn autoupdate(package_name: &str) {
    let package: Package = get_package(package_name.clone()).await;    
    let url = package.clone().autoupdate.download_page;

    if url.clone() == "" {
        // no autoupdate
        update_url_and_version(package.clone(), &package.latest_version.clone(), package_name).await;
        std::process::exit(0);
    }

    println!("url: {}", url);
    let response = get(url)
        .await
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));

    // println!("cont: {}", file_contents);

    let regex = regex::Regex::new(package.autoupdate.regex.as_str()).unwrap();

    let matches: Vec<&str> = regex
        .captures_iter(file_contents.as_str())
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    println!("matches: {:?}", matches);

    let mut versions_calc: Vec<String> = vec![];

    let mut versions: Vec<String> = vec![];
    let mut lengths: Vec<usize> = vec![];

    for mut _match in matches {
        _match = _match.trim_end();
        let version_split: Vec<&str> = _match.split(" ").collect();
        if version_split.len() > 1 {
            _match = version_split[1].trim();
            if _match.contains("v") {
                let version_split: Vec<&str> = _match.split("v").collect();
                _match = version_split[1].trim();
            }
        } else {
            _match = version_split[0];
            if _match.contains("v") {
                let version_split: Vec<&str> = _match.split("v").collect();
                _match = version_split[1].trim();
            }
        }
        lengths.push(_match.len());
        let ver = format!("{:0<15}", _match);
        versions.push(ver);
        let year_dot_split: Vec<&str> = _match.split(".").collect();
        let year_string = year_dot_split.concat();
        versions_calc.push(year_string);
    }

    println!("version final: {:?}", versions);

    let index = versions_calc
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.parse::<u64>().ok().map(|n| (i, n)))
        .max_by_key(|&(_, n)| n)
        .map(|(i, _)| i)
        .unwrap_or_else(|| handle_error_and_exit("Failed to find match".to_string()));

    let version = &versions[index];
    let og_len = &lengths[index];
    let ver = &version.split_at(*og_len).0;
    let version_new = &ver.to_string();
    println!("latest version: {}", version_new);

    if &package.latest_version != version_new {
        if package.clone().autoupdate.download_url == "" {
            update_version(package.clone(), &version_new, package_name);
        } else {
            update_url_and_version(package.clone(), &version_new, package_name).await;
        }
    }
}

fn remove(package_name: &str) {
    std::fs::remove_file(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json",
        package_name
    ))
    .unwrap();
    let package_list_loc = std::path::Path::new(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\package-list\package-list.json",
    );
    let file_contents = std::fs::read_to_string(package_list_loc).unwrap();
    let package_list: PackageList =
        serde_json::from_str::<PackageList>(file_contents.as_str()).unwrap();
    let mut packages: Vec<String> = package_list.packages;
    let index = packages.iter().position(|x| *x == package_name).unwrap();
    packages.remove(index);
    packages.sort();
    let package_list: PackageList = PackageList { packages: packages };
    let file = std::fs::File::create(r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\package-list\package-list.json").unwrap();
    to_writer_pretty(file, &package_list).unwrap();
}

fn update_version(package: Package, version: &str, package_name: &str) {
    let mut temp_package: Package = package.clone();
    temp_package.latest_version = version.to_string();

    let file = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json",
        package_name
    ))
    .unwrap();
    to_writer_pretty(file, &temp_package).unwrap();
    let mut commit = format!("autoupdate: {}", package_name);
    commit = "\"".to_string() + commit.as_str() + "\"";
    std::process::Command::new("powershell")
        .arg("novus_update")
        .output()
        .expect("Failed to update gcp bucket");
    let dir = std::path::Path::new(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\",
    );
    let _ = std::env::set_current_dir(dir);
    std::process::Command::new("powershell")
        .args(&["deploy", commit.as_str(), "main"])
        .output()
        .expect("Failed to deploy to github");
}

async fn update_url_and_version(package: Package, version: &str, package_name: &str) {
    let mut temp_package: Package = package.clone();
    let mut url = package.autoupdate.download_url.clone();
    let mut file_type: String = ".exe".to_string();
    if package.autoupdate.download_url.contains(".msi") {
        file_type = ".msi".to_string();
    }
    if package.autoupdate.download_url.contains("<version>") {
        url = url.replace("<version>", version);
    }
    if package.autoupdate.download_url.contains("<major-version>") {
        let version_split: Vec<&str> = version.split(".").collect();
        let mut version_new = String::new();
        if version_split.len() == 2 {
            version_new = version_split[0].to_string();
        }
        if version_split.len() == 3 {
            version_new = version_split[0].to_string() + "." + version_split[1];
        }
        if version_split.len() == 1 {
            version_new = version_split[0].to_string();
        }
        url = url.replace("<major-version>", version_new.as_str());
    }
    if package.autoupdate.download_url.contains("<minor-version>") {
        let version_split: Vec<&str> = version.split(".").collect();
        let mut version_new = String::new();
        if version_split.len() == 2 {
            version_new = version_split[1].to_string();
        }
        if version_split.len() == 3 {
            version_new = version_split[2].to_string();
        }
        if version_split.len() == 1 {
            version_new = version_split[0].to_string();
        }
        url = url.replace("<minor-version>", version_new.as_str());
    }
    if package.autoupdate.download_url.contains("<version-no-dot>") {
        url = url.replace("<version-no-dot>", &version.replace(".", ""));
    }
    println!("url: {}", url);
    let response = get(url.clone())
        .await
        .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_size = response
        .content_length()
        .unwrap_or_else(|| handle_error_and_exit("Failed to get content length".to_string()));

    let temp = std::env::var("TEMP").unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let loc = format!(r"{}\novus\{}_check{}", temp, package_name, file_type);
    threadeddownload(
        url.clone(),
        loc.clone(),
        package.threads,
        package_name.to_string(),
        "".to_string(),
        false,
        false,
    )
    .await;
    let hash = get_checksum(loc.clone());

    let _ = std::fs::remove_file(loc);

    let version_data: VersionData = VersionData {
        url: url,
        size: file_size,
        checksum: hash,
        file_type: file_type.clone(),
    };

    println!("version_data: {:?}", version_data);

    // make changes to data
    temp_package
        .versions
        .insert(version.clone().to_string(), version_data);
    temp_package.latest_version = version.to_string();

    // Re-open file to replace the contents:
    let file = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json",
        package_name
    ))
    .unwrap();
    to_writer_pretty(file, &temp_package).unwrap();
    let mut commit = format!("autoupdate: {}", package_name);
    commit = "\"".to_string() + commit.as_str() + "\"";
    std::process::Command::new("powershell")
        .arg("novus_update")
        .output()
        .expect("Failed to update gcp bucket");
    let dir = std::path::Path::new(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\",
    );
    let _ = std::env::set_current_dir(dir);
    std::process::Command::new("powershell")
        .args(&["deploy", commit.as_str(), "main"])
        .output()
        .expect("Failed to deploy to github");
}

fn handle_error_and_exit(e: String) -> ! {
    println!("{}{:?}", "error ".bright_red(), e);
    std::process::exit(0);
}

async fn get_packages() -> String {
    let response = get(format!(
        "https://storage.googleapis.com/novus_bucket/package-list/package-list.json?a={:?}",
        std::time::UNIX_EPOCH.elapsed().unwrap()
    ))
    .await
    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:36", e.to_string())));
    let content: Value = from_str(file_contents.as_str())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:53", e.to_string())));
    to_string_pretty(&content)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:54", e.to_string())))
}

async fn get_package(package_name: &str) -> Package {
    println!(
        "getting: https://storage.googleapis.com/novus_bucket/{}.json?a={:?}",
        package_name,
        std::time::UNIX_EPOCH.elapsed().unwrap()
    );
    let response = get(format!(
        "https://storage.googleapis.com/novus_bucket/{}.json?a={:?}",
        package_name,
        std::time::UNIX_EPOCH.elapsed().unwrap()
    ))
    .await
    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:36", e.to_string())));
    from_str::<Package>(&file_contents)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} get_package.rs:29", e.to_string())))
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
    let temp = std::env::var("TEMP")
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:106", e.to_string())));

    if max {
        let progress_bar = ProgressBar::new(total_length);
        progress_bar.set_style(ProgressStyle::default_bar()
              .template("[{elapsed_precise}] [{wide_bar:.cyan/blue/magenta}] {bytes}/{total_bytes} ({eta})")
              .progress_chars("=>-"));

        for index in 0..threads {
            let loc = format!(r"{}\novus\setup_{}{}.tmp", temp, package_name, index + 1);
            let (start, end) = get_splits(index + 1, total_length, threads);
            let pb = progress_bar.clone();
            let mut file = BufWriter::new(File::create(loc).unwrap_or_else(|e| {
                handle_error_and_exit(format!("{} install.rs:119", e.to_string()))
            }));
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| {
                        handle_error_and_exit(format!("{} install.rs:129", e.to_string()))
                    });

                while let Some(chunk) = response.chunk().await.unwrap_or_else(|e| {
                    handle_error_and_exit(format!("{} install.rs:134", e.to_string()))
                }) {
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
            let mut file = BufWriter::new(File::create(loc).unwrap_or_else(|e| {
                handle_error_and_exit(format!("{} install.rs:150", e.to_string()))
            }));
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| {
                        handle_error_and_exit(format!("{} install.rs:160", e.to_string()))
                    });
                while let Some(chunk) = response.chunk().await.unwrap_or_else(|e| {
                    handle_error_and_exit(format!("{} install.rs:164", e.to_string()))
                }) {
                    let _ = file.write(&*chunk);
                }
            }));
        }

        futures::future::join_all(handles).await;
    }

    let mut file = File::create(output.clone())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:175", e.to_string())));

    let temp = std::env::var("TEMP").unwrap();

    for index in 0..threads {
        let loc = format!(r"{}\novus\setup_{}{}.tmp", temp, package_name, index + 1);
        let mut buf: Vec<u8> = vec![];
        let downloaded_file = File::open(loc.clone())
            .unwrap_or_else(|e| handle_error_and_exit(format!("{} install.rs:183", e.to_string())));
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
