// #[path = "./classes/auto_update.rs"]
// mod auto_update;
// #[path = "./classes/package_v1.rs"]
// mod package_v1;
// #[path = "./classes/package_v2.rs"]
// mod package_v2;
// #[path = "./classes/version_data.rs"]
// mod version_data;

use classes::auto_update::AutoUpdateData;
use classes::package_v1::Packagev1;
use classes::package_v2::Package;
use classes::version_data::VersionData;
use clipboard_win::{formats, set_clipboard};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::get;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string_pretty, to_writer_pretty, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
// use std::fs;
use std::fs::{copy, read_dir, remove_dir_all, remove_file};
use std::io::{BufReader, BufWriter, Write};
// use std::path::PathBuf;
use std::{fs::File, u64};
use zip::ZipArchive;

const PACKAGE_VERSIONS: i32 = 2;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    // println!("args: {:?}", args);
    let data = get_packages().await;

    let val = data
        .as_str()
        .parse::<Value>()
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

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

    // add_portable(package_list).await;
    // add_aliases(package_list).await;

    if args.len() == 1 {
        // println!("pkg: {:?}", package_list);
        for package in package_list {
            println!("Checking {}", package);
            autoupdate(package).await;
        }
    } else {
        if args[1] == "add" {
            new_package(&args[2]);
        } else if args[1] == "test" {
            get_contents(&args[2]).await;
        } else if args[1] == "remove" {
            remove(&args[2]);
        } else if args[1] == "update" {
            update_package(&args[2], &args[3], &args[4]).await;
        } else if args[1] == "mirror" {
            mirror_package(&args[2]).await;
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
    // println!("url: {}", url);
    let response = get(url)
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    // println!("cont: {}", file_contents);

    set_clipboard(formats::Unicode, file_contents).expect("To set clipboard");
}

fn new_package(package_name: &String) {
    for version_index in 1..PACKAGE_VERSIONS + 1 {
        let loc = format!(
            r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v{}\{}.json",
            version_index, package_name
        );
        let path = std::path::Path::new(&loc);
        let loc = format!(
            r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v{}\package-list\package-list.json",
            version_index
        );
        let package_list_loc = std::path::Path::new(&loc);
        let file_contents = std::fs::read_to_string(package_list_loc).unwrap();
        let package_list: PackageList =
            serde_json::from_str::<PackageList>(file_contents.as_str()).unwrap();
        let mut packages: Vec<String> = package_list.packages;
        packages.push(package_name.clone());
        packages.sort();
        let package_list: PackageList = PackageList { packages: packages };
        let file = std::fs::File::create(format!(r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v{}\package-list\package-list.json", version_index)).unwrap();
        to_writer_pretty(file, &package_list).unwrap();
        let package_file = std::fs::File::create(path).unwrap();
        if version_index == 1 {
            let package: Packagev1 = Packagev1 {
                package_name: package_name.clone(),
                display_name: String::new(),
                exec_name: "none".to_string(),
                portable: Some(false),
                creator: String::new(),
                description: String::new(),
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
        } else if version_index == 2 {
            let package: Package = Package {
                package_name: package_name.clone(),
                display_name: String::new(),
                aliases: vec![package_name.clone()],
                exec_name: "none".to_string(),
                portable: Some(false),
                creator: String::new(),
                description: String::new(),
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
    }
}

async fn autoupdate(package_name: &str) {
    let package: Package = get_package(package_name.clone()).await;
    let packagev1: Packagev1 = get_package_v1(package_name.clone()).await;
    let url = package.clone().autoupdate.download_page;

    if url.clone() != "" {
        // println!("url: {}", url);
        let response = get(url).await.unwrap_or_else(|e| {
            handle_error_and_exit(format!("{}: line {}", e.to_string(), line!()))
        });

        let mut month = "";
        let mut year = "";
        let mut date = "";
        #[allow(unused_assignments)]
        let mut date_match: String = "null".to_string();

        let file_contents = response.text().await.unwrap_or_else(|e| {
            handle_error_and_exit(format!("{}: line {}", e.to_string(), line!()))
        });

        // println!("cont: {}", file_contents);

        let regex = regex::Regex::new(package.autoupdate.regex.as_str()).unwrap();
        let mut new_match;

        for captures in regex.captures_iter(file_contents.as_str()) {
            // println!("Captures: {:?}", captures);
            if captures.len() > 2 {
                for cap in captures.iter() {
                    let cap = &cap.unwrap().as_str();
                    if MONTHS.contains(&cap.to_lowercase().as_str()) {
                        month = month_to_number(cap);
                    } else if cap.len() == 4 {
                        year = cap;
                    } else {
                        date = cap;
                    }
                }
            }
        }

        if year != "" && month != "" && date != "" {
            date_match = year.to_string() + "." + month + "." + date;
        }

        let matches: Vec<&str>;

        if date_match == "null".to_string() {
            matches = regex
                .captures_iter(file_contents.as_str())
                .map(|c| c.get(1).unwrap().as_str())
                .collect();

            for mut _match in matches.clone() {
                for month in MONTHS.iter() {
                    if _match.contains(month) {
                        let number = month_to_number(month);
                        new_match = _match.replace(month, number);
                        _match = &new_match;
                    }
                }
            }
        } else {
            matches = vec![&date_match];
        }

        // println!("matches: {:?}", matches);

        if matches.len() != 0 {
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
                let year_dash_split: Vec<&str> = year_string.split("-").collect();
                let year_string = year_dash_split.concat();

                versions_calc.push(year_string);
            }

            println!("version final: {:?}", versions_calc);

            let mut new_versions_calc = vec![];

            for v in versions_calc.clone() {
                new_versions_calc.push(parse_number_with_letters(&v).unwrap_or_else(|_| {
                    handle_error_and_exit(format!("Failed to parse version for {}", package_name))
                }));
            }

            let max = new_versions_calc.iter().max().unwrap_or_else(|| {
                handle_error_and_exit("Failed to get max value of vector".to_string())
            });

            let index = new_versions_calc.iter().position(|&r| &r == max).unwrap();

            // let index = new_versions_calc
            //     .iter()
            //     .enumerate()
            //     // .filter_map(|(i, s)| s.parse::<u64>().ok().map(|n| (i, n)))
            //     .max_by_key(|&(_, n)| n)
            //     .map(|(i, _)| i)
            //     .unwrap_or_else(|| handle_error_and_exit("Failed to find match".to_string()));

            let version = &versions[index];
            let og_len = &lengths[index];
            let ver = &version.split_at(*og_len).0;
            let version_new = &ver.to_string();
            // println!("latest version: {}", version_new);
            if &package.latest_version != version_new {
                if package.clone().autoupdate.download_url == "" {
                    update_version(package.clone(), packagev1, &version_new, package_name);
                } else {
                    update_url_and_version(package.clone(), packagev1, &version_new, package_name)
                        .await;
                }
            }
        }
    }
}

fn month_to_number(month: &str) -> &str {
    let month: &str = &month.to_lowercase();
    match month {
        "january" => "1",
        "february" => "2",
        "march" => "3",
        "april" => "4",
        "may" => "5",
        "june" => "6",
        "july" => "7",
        "august" => "8",
        "september" => "9",
        "october" => "10",
        "november" => "11",
        "december" => "12",
        _ => {
            println!("{}", "Failed to convert month to number".bright_red());
            std::process::exit(1);
        }
    }
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn parse_number_with_letters(s: &str) -> Result<u64, std::num::ParseIntError> {
    let with_letters_replaced: String = s
        .chars()
        .map(|c| letter_to_number(c).unwrap_or(c.to_string()))
        .collect();

    // println!("letters replaced: {}", with_letters_replaced);

    with_letters_replaced.trim().parse::<u64>()
}

fn letter_to_number(c: char) -> Option<String> {
    let number = match c {
        'a' => "1".to_string(),
        'b' => "2".to_string(),
        'c' => "3".to_string(),
        'd' => "4".to_string(),
        'e' => "5".to_string(),
        'f' => "6".to_string(),
        'g' => "7".to_string(),
        'h' => "8".to_string(),
        'i' => "9".to_string(),
        'r' => "18".to_string(),
        _ => return None,
    };

    Some(number)
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

fn update_version(package: Package, packagev1: Packagev1, version: &str, package_name: &str) {
    let mut temp_package: Package = package.clone();
    let mut temp_package_v1: Packagev1 = packagev1.clone();

    temp_package.latest_version = version.to_string();
    temp_package_v1.latest_version = version.to_string();

    let file = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v2\{}.json",
        package_name
    ))
    .unwrap();
    let file_v1 = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v1\{}.json",
        package_name
    ))
    .unwrap();

    to_writer_pretty(file, &temp_package).unwrap();
    to_writer_pretty(file_v1, &temp_package_v1).unwrap();

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

async fn update_url_and_version(
    package: Package,
    packagev1: Packagev1,
    version: &str,
    package_name: &str,
) {
    let mut temp_package: Package = package.clone();
    let mut temp_package_v1: Packagev1 = packagev1.clone();
    let mut url = package.autoupdate.download_url.clone();

    // println!("Response Status -> {}", response_status);
    let portable = package.portable.unwrap_or(false);
    let mut file_type: String = ".exe".to_string();
    if package.autoupdate.download_url.contains(".msi") {
        file_type = ".msi".to_string();
    }
    if package.autoupdate.download_url.contains(".zip") || portable {
        file_type = ".zip".to_string();
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
    if package
        .autoupdate
        .download_url
        .contains("<major-version-no-dot>")
    {
        let version_split: Vec<&str> = version.split(".").collect();
        let mut version_new = String::new();
        if version_split.len() == 2 {
            version_new = version_split[0].to_string();
        }
        if version_split.len() == 3 {
            version_new = version_split[0].to_string() + version_split[1];
        }
        if version_split.len() == 1 {
            version_new = version_split[0].to_string();
        }
        url = url.replace("<major-version-no-dot>", version_new.as_str());
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
    // println!("url: {}", url);
    let response = get(url.clone())
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    // println!("response status: {:?}", response.status());
    let file_size = response.content_length().unwrap_or(10000);

    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    let mut loc = format!(r"{}\novus\{}_check{}", appdata, package_name, file_type);

    // println!("Downloading from url\n    -> {}\n    -> {}", url.clone().bright_cyan(), loc.clone().bright_magenta());

    if response.status() == 200 {
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

        if file_type == ".zip" && !portable {
            let (filetype_temp, loc_temp) =
                extract_file(loc.clone(), appdata, package_name.to_string());
            file_type = filetype_temp;
            loc = loc_temp;
        }

        // println!("filetype: {}\n loc: {};", file_type, loc.clone());

        let hash = get_checksum(loc.clone());

        let _ = std::fs::remove_file(loc).unwrap_or_else(|e| {
            handle_error_and_exit(format!("{}: line {}", e.to_string(), line!()))
        });

        let version_data: VersionData = VersionData {
            url: url.clone(),
            size: file_size,
            checksum: hash,
            file_type: file_type.clone(),
        };

        // make changes to data
        temp_package
            .versions
            .insert(version.clone().to_string(), version_data.clone());
        temp_package.latest_version = version.to_string();

        temp_package_v1
            .versions
            .insert(version.clone().to_string(), version_data);
        temp_package_v1.latest_version = version.to_string();

        // Re-open file to replace the contents:
        let file = std::fs::File::create(format!(
                r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v2\{}.json",
                package_name
            )).unwrap();

        let file_v1 = std::fs::File::create(format!(
                r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v1\{}.json",
                package_name
            ))
            .unwrap();

        to_writer_pretty(file, &temp_package).unwrap();
        to_writer_pretty(file_v1, &temp_package_v1).unwrap();
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
        println!(
            "{} {} {} {}",
            "Updated".bright_green(),
            package_name.bright_green(),
            "to".bright_green(),
            version.bright_green()
        );
    } else {
        println!(
            "{} {}\n    -> {}",
            "Detected Corrupted Dowload For".bright_red(),
            package_name.bright_red(),
            url.bright_cyan()
        );
    }
}

fn extract_file(loc: String, appdata: String, package_name: String) -> (String, String) {
    // Extract exe from package

    let zip_file = File::open(loc.clone())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    let mut archive = ZipArchive::new(zip_file)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    let extract_dir = format!(r"{}\novus\{}_check", appdata, package_name);

    archive
        .extract(&extract_dir)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    let mut path: String = String::new();

    for entry in read_dir(std::path::Path::new(&extract_dir))
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())))
    {
        let entry = entry.unwrap_or_else(|e| {
            handle_error_and_exit(format!("{}: line {}", e.to_string(), line!()))
        });
        path = entry.path().display().to_string();
    }

    let mut filetype = ".exe";

    if path.contains(".msi") {
        filetype = ".msi";
    }

    let copy_dir = format!(r"{}\novus\{}_check{}", appdata, package_name, filetype);

    copy(path, copy_dir.clone())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    remove_dir_all(extract_dir)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    remove_file(loc.clone())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    (filetype.to_string(), copy_dir)
}

fn handle_error_and_exit(e: String) -> ! {
    println!("{}{:?}", "error ".bright_red(), e);
    std::process::exit(0);
}

async fn get_packages() -> String {
    let response = get(format!(
        "https://storage.googleapis.com/novus_bucket/packages_v2/package-list/package-list.json?a={:?}",
        std::time::UNIX_EPOCH.elapsed().unwrap()
    ))
    .await
    .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    let content: Value = from_str(file_contents.as_str())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    to_string_pretty(&content)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())))
}

#[allow(dead_code)]
async fn get_package_v1(package_name: &str) -> Packagev1 {
    // println!(
    //     "getting: https://storage.googleapis.com/novus_bucket/{}.json?a={:?}",
    //     package_name,
    //     std::time::UNIX_EPOCH.elapsed().unwrap()
    // );
    let response = get(format!(
        "https://storage.googleapis.com/novus_bucket/packages_v1/{}.json?a={:?}",
        package_name,
        std::time::UNIX_EPOCH.elapsed().unwrap()
    ))
    .await
    .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
    let file_contents = response
        .text()
        .await
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    from_str::<Packagev1>(&file_contents)
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())))
}

async fn get_package(package_name: &str) -> Package {
    // println!(
    //     "getting: https://storage.googleapis.com/novus_bucket/{}.json?a={:?}",
    //     package_name,
    //     std::time::UNIX_EPOCH.elapsed().unwrap()
    // );
    let response = get(format!(
        "https://storage.googleapis.com/novus_bucket/packages_v2/{}.json?a={:?}",
        package_name,
        std::time::UNIX_EPOCH.elapsed().unwrap()
    ))
    .await
    .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));
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
    let total_length = res.content_length().unwrap_or(10000);
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    if max {
        let progress_bar = ProgressBar::new(total_length);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    ("Downloading".bright_cyan().to_string()
                        + " [{wide_bar:.cyan}] {bytes}/{total_bytes}")
                        .as_str(),
                )
                .progress_chars("=> "),
        );

        for index in 0..threads {
            let loc = format!(r"{}\novus\setup_{}{}.tmp", appdata, package_name, index + 1);
            let (start, end) = get_splits(index + 1, total_length, threads);
            let pb = progress_bar.clone();
            let mut file = BufWriter::new(
                File::create(loc).unwrap_or_else(|e| handle_error_and_exit(e.to_string())),
            );
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));

                while let Some(chunk) = response
                    .chunk()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()))
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
            let loc = format!(r"{}\novus\setup_{}{}.tmp", appdata, package_name, index + 1);
            let (start, end) = get_splits(index + 1, total_length, threads);
            let mut file = BufWriter::new(
                File::create(loc)
                    .unwrap_or_else(|e| handle_error_and_exit(format!("{}", e.to_string()))),
            );
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let client = reqwest::Client::new();
                let mut response = client
                    .get(url)
                    .header("range", format!("bytes={}-{}", start, end))
                    .send()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()));
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .unwrap_or_else(|e| handle_error_and_exit(e.to_string()))
                {
                    let _ = file.write(&*chunk);
                }
            }));
        }

        futures::future::join_all(handles).await;
    }

    let mut file = File::create(output.clone())
        .unwrap_or_else(|e| handle_error_and_exit(format!("{}: line {}", e.to_string(), line!())));

    let appdata = std::env::var("APPDATA").unwrap();

    for index in 0..threads {
        let loc = format!(r"{}\novus\setup_{}{}.tmp", appdata, package_name, index + 1);
        let mut buf: Vec<u8> = vec![];
        let downloaded_file = File::open(loc.clone()).unwrap_or_else(|e| {
            handle_error_and_exit(format!("{}: line {}", e.to_string(), line!()))
        });
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

// async fn _add_portable(package_list: Vec<&str>) {
//     for pkg in package_list {
//         let package: Package = get_package(pkg.clone()).await;
//         let mut temp_package: Package = package.clone();
//         temp_package.portable = Some(false);
//         let file = std::fs::File::create(format!(
//             r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\{}.json",
//             pkg
//         ))
//         .unwrap();
//         to_writer_pretty(file, &temp_package).unwrap();
//     }
// }
async fn update_package(package_name: &str, field: &str, value: &str) {
    let mut package: Package = get_package(&package_name).await;
    match field {
        "package_name" => package.package_name = value.to_string(),
        "display_name" => package.display_name = value.to_string(),
        "aliases" => package.aliases = vec![value.to_string()],
        "exec_name" => package.exec_name = value.to_string(),
        "creator" => package.creator = value.to_string(),
        "description" => package.description = value.to_string(),
        "threads" => package.threads = value.parse().unwrap_or(8),
        "download_page" => package.autoupdate.download_page = value.to_string(),
        "download_url" => package.autoupdate.download_url = value.to_string(),
        "regex" => package.autoupdate.regex = value.to_string(),
        &_ => {}
    }

    let file = std::fs::File::create(format!(
            r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v2\{}.json",
            package_name
        ))
        .unwrap();
    to_writer_pretty(file, &package).unwrap();

    let mut packagev1: Packagev1 = get_package_v1(&package_name).await;
    match field {
        "package_name" => packagev1.package_name = value.to_string(),
        "display_name" => packagev1.display_name = value.to_string(),
        "exec_name" => packagev1.exec_name = value.to_string(),
        "creator" => packagev1.creator = value.to_string(),
        "description" => packagev1.description = value.to_string(),
        "threads" => packagev1.threads = value.parse().unwrap_or(8),
        "download_page" => packagev1.autoupdate.download_page = value.to_string(),
        "download_url" => packagev1.autoupdate.download_url = value.to_string(),
        "regex" => packagev1.autoupdate.regex = value.to_string(),
        &_ => {}
    }

    let file = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v1\{}.json",
        package_name
    ))
    .unwrap();
    to_writer_pretty(file, &packagev1).unwrap();

    std::process::Command::new("powershell")
        .arg("novus_update")
        .output()
        .expect("Failed to update gcp bucket");
}

async fn mirror_package(package_name: &str) {
    let package: Package = get_package(&package_name).await;
    let mut packagev1: Packagev1 = get_package_v1(&package_name).await;
    packagev1.display_name = package.display_name;
    packagev1.package_name = package.package_name;
    packagev1.exec_name = package.exec_name;
    packagev1.portable = package.portable;
    packagev1.creator = package.creator;
    packagev1.description = package.description;
    packagev1.latest_version = package.latest_version;
    packagev1.threads = package.threads;
    packagev1.iswitches = package.iswitches;
    packagev1.uswitches = package.uswitches;
    packagev1.autoupdate = package.autoupdate;
    packagev1.versions = package.versions;

    let file = std::fs::File::create(format!(
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages\packages_v1\{}.json",
        package_name
    ))
    .unwrap();
    to_writer_pretty(file, &packagev1).unwrap();

    let mut commit = format!(
        "autoupdate: mirrored {} to all package versions",
        package_name
    );
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
    println!(
        "{} {} {}",
        "Mirrored".bright_green(),
        package_name.bright_green(),
        "to all package versions".bright_green()
    );
}

async fn _add_aliases(package_list: Vec<&str>) {
    let packages_v2_dir =
        r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages_v2";
    let _ = std::fs::create_dir(std::path::Path::new(packages_v2_dir));
    for pkg in package_list {
        let package: Packagev1 = get_package_v1(pkg.clone()).await;
        // let version_data: VersionData = package.versions[package.latest_version];
        // let mut temp_package: Package = package.clone();
        let temp_package: Package = Package {
            package_name: package.package_name.clone(),
            display_name: package.display_name,
            aliases: vec![package.package_name.clone()],
            exec_name: package.exec_name,
            portable: package.portable,
            creator: package.creator,
            description: package.description,
            latest_version: package.latest_version,
            threads: package.threads,
            iswitches: package.iswitches,
            uswitches: package.uswitches,
            autoupdate: AutoUpdateData {
                download_page: package.autoupdate.download_page,
                download_url: package.autoupdate.download_url,
                regex: package.autoupdate.regex,
            },
            versions: package.versions,
        };
        // temp_package.aliases = vec![package.package_name];
        let file = std::fs::File::create(format!(
            r"D:\prana\Programming\My Projects\novus-package-manager\novus-packages\packages_v2\{}.json",
            pkg
        ))
        .unwrap();
        to_writer_pretty(file, &temp_package).unwrap();
    }
}
