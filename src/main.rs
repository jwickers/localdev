use clap::{IntoApp, Parser};
use clap_complete::{generate, Generator};
use colored::*;
use regex::Regex;
use std::{collections::HashMap, io::BufWriter, io::Write, path::Path};
mod cli;

fn mkcert(nginx_dir_path: &Path, name: &str, verbose: bool) {
    if verbose {
        println!("Running mkcert ...");
    }
    let dir_p = nginx_dir_path.display();
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "mkcert -cert-file {}/{}.pem -key-file {}/{}-key.pem {}",
            dir_p, name, dir_p, name, name
        ))
        .output()
        .unwrap();
    // print the output
    if verbose {
        println!("mkcert status ? {}", output.status);
        println!("{}", String::from_utf8(output.stdout).unwrap());
    }
    assert!(output.status.success());
}

fn reload_nginx(verbose: bool) {
    if verbose {
        println!("Running nginx reload ...");
    }
    let output = std::process::Command::new("nginx")
        .arg("-s")
        .arg("reload")
        .output()
        .unwrap();
    if verbose {
        println!("nginx reload status ? {}", output.status);
        println!("{}", String::from_utf8(output.stdout).unwrap());
    }
    assert!(output.status.success(), "Failed to reload nginx");
}

fn open_server(server_name: &str) {
    let url = format!("https://{}", server_name);
    println!("");
    println!(" ⚡ Opening {}", url);
    webbrowser::open(&url).expect("failed to open URL");
}

fn print_server(
    server_name: &str,
    server_name_to_proxies: &HashMap<String, HashMap<String, String>>,
) {
    println!("");
    let s = format!("https://{}", server_name);
    println!(" 🚦 {}", s.bold());
    // sort the proxies by location
    let proxies_map = server_name_to_proxies.get(server_name).unwrap();
    let mut proxies: Vec<&String> = proxies_map.keys().collect();
    proxies.sort();
    // align the output according to the longest location
    let l = proxies.iter().map(|x| x.len()).max().unwrap();
    for location in proxies {
        let target = proxies_map.get(location).unwrap().trim().trim_matches('/');
        let location = location.trim().trim_matches('/');
        // padd the location to the longest location l
        let location = format!("/{:<l$}", location, l = l);
        println!("     🚀 {}=> {}", location.green(), target.blue());
    }
}

/// Find server_name or with added extension .localdev in the parsed config.
fn find_server_name(server_name: &str, server_names: &[String]) -> Option<String> {
    let mut name_local: String = server_name.to_owned();
    name_local.push_str(".localdev");
    let found = server_names
        .iter()
        .find(|n| n == &&server_name || n == &&name_local);
    match found {
        Some(n) => Some(n.to_owned()),
        None => None,
    }
}

/// Generate the auto complete for the CLI
fn print_completer<G: Generator>(generator: G) {
    let mut app = cli::Args::command();
    let name = app.get_name().to_owned();

    generate(generator, &mut app, name, &mut std::io::stdout());
}

/// Write helper for the proxy location header
fn write_location_header<T: std::io::Write>(
    f: &mut BufWriter<T>,
    location: &str,
    is_websocket: bool,
) {
    f.write_all(b"  location ").unwrap();
    if !location.starts_with("/") {
        f.write_all(b"/").unwrap();
    }
    write!(f, "{}", location).unwrap();
    if !is_websocket && !location.ends_with("/") {
        f.write_all(b"/").unwrap();
    }
    f.write_all(b" {\n").unwrap();
}

/// Write helper for the proxy section
fn write_proxy<T: std::io::Write>(f: &mut BufWriter<T>, location: &str, target: &str) {
    write_location_header(f, location, false);
    write!(f, "      proxy_pass {}", target).unwrap();
    if !target.ends_with("/") {
        f.write_all(b"/").unwrap();
    }
    f.write_all(b";\n").unwrap();
    f.write_all(b"  }\n").unwrap();
}

/// Write helper for the websocket proxy section
fn write_websocket_proxy<T: std::io::Write>(f: &mut BufWriter<T>, location: &str, name: &str) {
    write_location_header(f, location, true);
    f.write_all(b"    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n")
        .unwrap();
    f.write_all(b"    proxy_set_header Host $host;\n").unwrap();
    write!(f, "    proxy_pass http://ws-backend-{};\n", name).unwrap();
    f.write_all(b"    proxy_http_version 1.1;\n").unwrap();
    f.write_all(b"    proxy_set_header Upgrade $http_upgrade;\n")
        .unwrap();
    f.write_all(b"    proxy_set_header Connection \"upgrade\";\n")
        .unwrap();
    f.write_all(b"  }\n").unwrap();
}

/// Write helper for the upstream websocket section
fn write_websocket_upstream<T: std::io::Write>(f: &mut BufWriter<T>, upstream: &str, name: &str) {
    write!(f, "upstream ws-backend-{} {{\n", name).unwrap();
    f.write_all(b"  ip_hash;\n").unwrap();
    write!(f, "  server {};\n", upstream).unwrap();
    f.write_all(b"}\n").unwrap();
}

fn parse_proxy_arg(arg: &str, with_protocol: bool) -> Option<(String, String)> {
    // split the string separated by =
    let mut split = arg.splitn(2, "=").collect::<Vec<&str>>();
    if split.len() < 2 {
        // can also split on :
        split = arg.splitn(2, ":").collect::<Vec<&str>>();
    }
    if split.len() == 2 {
        let port_target_rx = Regex::new(r"^:?([0-9]+.*)$").unwrap();
        let mut location = split[0].trim().to_string();
        if !location.starts_with("/") {
            location = String::from(format!("/{}", location));
        }
        let mut target = split[1].trim().to_string();
        // if the target matches port_target_rx
        if let Some(caps) = port_target_rx.captures(&target) {
            let port = caps.get(1).unwrap().as_str();
            if with_protocol {
                target = String::from(format!("http://localhost:{}", port));
            } else {
                target = String::from(format!("localhost:{}", port));
            }
        }

        return Some((location, target));
    } else {
        println!("❗ Invalid proxy: {}", arg);
        return None;
    }
}

fn main() {
    let args = cli::Args::parse();

    // a list of possible paths for the file, could be /etc/nginx/nginx.conf or /usr/local/etc/nginx/nginx.conf
    let paths: Vec<&str>;
    if let Some(path) = args.nginx_path.as_deref() {
        paths = vec![&path];
    } else {
        paths = vec!["/etc/nginx/nginx.conf", "/usr/local/etc/nginx/nginx.conf"];
    }
    // find the first file that exists
    let path = paths.iter().find(|p| {
        let path = Path::new(p);
        path.exists() && path.is_file()
    });
    // exit if not found
    if path.is_none() {
        println!("Could not find nginx.conf");
        return;
    }
    // if we found one, print it
    let nginx_path = path.unwrap();
    let nginx_dir_path = Path::new(nginx_path).parent().unwrap();
    // will store the list of directories we found
    let mut found_dirs = vec![];
    if args.verbose > 0 {
        println!("Found nginx.conf at {}", nginx_path);
    }
    let include_rx = Regex::new(r"include\s+([a-zA-Z0-9]+)/\*").unwrap();
    // read the file
    let contents = std::fs::read_to_string(nginx_path).unwrap();
    // get the lines that match the pattern "include" and are not comments
    for line in contents.lines() {
        if line.trim_start().starts_with("include") {
            // check if it is a recursive include of a directory: matches regex include_rx
            let caps = include_rx.captures(line);
            if let Some(c) = caps {
                let dir = c.get(1).unwrap().as_str();
                // add to found_dirs
                found_dirs.push(dir);
            }
        }
    }
    // if we found only one dir
    if found_dirs.len() == 1 {
        // print it
        if args.verbose > 0 {
            println!("Found directory: {}", found_dirs[0]);
        }
    } else {
        // print all the found_dirs
        println!("Found more than one potential directory!");
        for dir in found_dirs {
            println!("{}", dir);
        }
        // exit
        std::process::exit(1);
    }

    // join nginx_path and found_dir
    let found_dir = nginx_dir_path.join(found_dirs[0]);

    let server_name_rx = Regex::new(r"server_name\s+(.*)\s*;").unwrap();
    let location_rx = Regex::new(r"location\s+(.*)\s*\{").unwrap();

    // store list of server names
    let mut server_names = vec![];
    // save a Map of server_name to path
    let mut server_name_to_path = HashMap::new();
    // save a Map of server_name to a list of proxies
    let mut server_name_to_proxies = HashMap::new();

    // check all the files in found_dir
    for p in found_dir.read_dir().unwrap() {
        let child = p.unwrap().path();
        // check if it is a file or directory
        if child.is_file() {
            if args.verbose > 0 {
                println!("Processing FILE: {}", child.display());
            }
            let mut proxies = HashMap::new();
            // read the file
            let contents = std::fs::read_to_string(child.clone()).unwrap();
            // get the lines starting with "server_name"
            let mut server_name: Option<&str> = None;
            let mut location: Option<&str> = None;
            for line in contents.lines() {
                let line = line.trim_start();
                if line.starts_with("server_name") {
                    let caps = server_name_rx.captures(line);
                    if let Some(c) = caps {
                        server_name = Some(c.get(1).unwrap().as_str());
                        // add to found_dirs
                        server_names.push(server_name.unwrap().to_owned());
                        server_name_to_path.insert(server_name.unwrap().to_owned(), child.clone());
                    }
                }
                if line.starts_with("location") {
                    let caps = location_rx.captures(line);
                    if let Some(c) = caps {
                        location = Some(c.get(1).unwrap().as_str());
                    }
                }
                if line.starts_with("proxy_pass") {
                    // add to proxies for the current location
                    match location {
                        Some(l) => {
                            // get the target
                            let target = line
                                .split_whitespace()
                                .nth(1)
                                .unwrap()
                                .trim()
                                .trim_end_matches(';')
                                .trim();
                            proxies.insert(l.to_owned(), target.to_owned());
                        }
                        None => {
                            if args.verbose > 0 {
                                println!("No current location for proxy_pass: {}", line);
                            }
                        }
                    }
                }
                if line.starts_with("}") {
                    if location.is_some() {
                        location = None;
                    }
                }
            }
            // if server_name str is initialized
            if server_name.is_some() {
                server_name_to_proxies.insert(server_name.unwrap().to_owned(), proxies);
            }
        }
    }
    // only care about the server names ending with .localdev domains
    server_names = server_names
        .into_iter()
        .filter(|s| s.ends_with(".localdev"))
        .collect();
    // remove duplicates from server_names
    server_names.sort();
    server_names.dedup();
    match args.command {
        Some(cli::Commands::Completion { shell }) => {
            print_completer(shell);
            return;
        }
        Some(cli::Commands::Reload {}) => {
            reload_nginx(args.verbose > 0);
            return;
        }
        Some(cli::Commands::Open { server_name }) => {
            let found = find_server_name(&server_name, server_names.as_slice());
            match found {
                Some(f) => {
                    // if open is set, open the serer in the browser
                    open_server(&f);
                    return;
                }
                None => {
                    println!("❗ Server name not found: {}", server_name);
                    println!("❗  Use the add command to create it.");
                    return;
                }
            }
        }
        Some(cli::Commands::Find { server_name, open }) => {
            let found = find_server_name(&server_name, server_names.as_slice());
            match found {
                Some(f) => {
                    // filter the server_names, those are printed below as the default command
                    server_names = server_names.into_iter().filter(|n| n == &f).collect();
                    // if open is set, open the serer in the browser
                    if open {
                        open_server(&f);
                    }
                }
                None => {
                    println!("❗ Server name not found: {}", server_name);
                    println!("❗  Use the add command to create it.");
                    return;
                }
            }
        }
        Some(cli::Commands::Remove { server_name }) => {
            let found = find_server_name(&server_name, server_names.as_slice());
            match found {
                Some(f) => {
                    println!("Removing current configuration for: {}", f);
                    let path_to_file = server_name_to_path[&f].to_owned();
                    //remove the file
                    std::fs::remove_file(path_to_file).unwrap();
                    reload_nginx(args.verbose > 0);
                }
                None => {
                    println!("Server name not found: {}", server_name);
                }
            }
            return;
        }
        Some(cli::Commands::Add {
            server_name,
            default_target,
            ws,
            proxy,
            force,
            open,
        }) => {
            let found = find_server_name(&server_name, server_names.as_slice());
            match found {
                Some(f) => {
                    if !force {
                        println!("❗ This server already exists: {}", f);
                        println!("❗  use --force to reconfigure");
                        return;
                    }
                }
                _ => {
                    println!("Server name not found: {}", server_name);
                }
            }

            // parse the websocket param
            let mut websocket: Option<(String, String)> = None;
            if !ws.is_empty() {
                websocket = parse_proxy_arg(&ws, false);
            }
            let (ws_l, ws_t) = match websocket {
                Some((w, l)) => (Some(w), Some(l)),
                None => (None, None),
            };

            // if there is no domain auto add the .devlocal to server_name
            let mut name = server_name;
            if !name.ends_with(".localdev") {
                name.push_str(".localdev");
            }
            if args.verbose > 0 {
                println!("No current configuration for server: {}", name);
            }
            // generate the SSL ssl_certificates using mkcert
            mkcert(nginx_dir_path, &name, args.verbose > 0);

            let mut proxies = HashMap::new();
            proxies.insert(String::from("/"), default_target.clone());

            // test proxy arg
            if !proxy.is_empty() {
                for p in proxy.iter() {
                    let res = parse_proxy_arg(p, true);
                    if let Some((location, target)) = res {
                        proxies.insert(location, target);
                    }
                }
            }

            if args.verbose > 0 {
                for (location, target) in proxies.iter() {
                    println!("Location: {}", location);
                    println!("Target: {}", target);
                }
            }

            // add it
            server_names.push(name.to_owned());
            let mut file_name = name.to_owned();
            file_name.push_str(".conf");
            let new_path = found_dir.join(file_name);

            // write to new_path
            {
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(new_path.clone())
                    .expect("unable to open file");
                let mut f = BufWriter::new(file);

                // add the HTTP proxy
                f.write_all(b"server {\n").unwrap();
                f.write_all(b"  listen 80;\n").unwrap();
                f.write_all(b"  listen [::]:80;\n").unwrap();
                write!(f, "  server_name {};\n", name).unwrap();
                for (location, target) in proxies.iter() {
                    write_proxy(&mut f, location, target);
                }
                // add a websocket proxy
                if ws_l.is_some() && ws_t.is_some() {
                    write_websocket_proxy(&mut f, ws_l.as_deref().unwrap(), &name);
                }
                f.write_all(b"}\n").unwrap();

                // write the SSL version
                f.write_all(b"server {\n").unwrap();
                f.write_all(b"  listen 443 ssl;\n").unwrap();
                write!(f, "  server_name {};\n", name).unwrap();
                write!(f, "  ssl_certificate      {}.pem;\n", name).unwrap();
                write!(f, "  ssl_certificate_key  {}-key.pem;\n", name).unwrap();
                f.write_all(b"  ssl_session_cache    shared:SSL:1m;\n")
                    .unwrap();
                f.write_all(b"  ssl_session_timeout  5m;\n").unwrap();
                f.write_all(b"  ssl_ciphers  HIGH:!aNULL:!MD5;\n").unwrap();
                f.write_all(b"  ssl_prefer_server_ciphers  on;\n").unwrap();
                for (location, target) in proxies.iter() {
                    write_proxy(&mut f, location, target);
                }
                // add a websocket proxy
                if ws_l.is_some() && ws_t.is_some() {
                    write_websocket_proxy(&mut f, ws_l.as_deref().unwrap(), &name);
                }
                f.write_all(b"}\n").unwrap();

                // add the upstream websocket server
                if ws_l.is_some() && ws_t.is_some() {
                    write_websocket_upstream(&mut f, ws_t.as_deref().unwrap(), &name);
                    proxies.insert(ws_l.unwrap(), format!("ws-backend-{}", &name));
                }

                // done writing
                f.flush().unwrap();
            }

            server_name_to_path.insert(name.to_owned(), new_path.clone());
            if args.verbose > 0 {
                println!(">> Wrote new configuration for server: {}", name);
            }
            // finally reload the nginx server using homebrew
            // or using nginx -s reload
            reload_nginx(args.verbose > 0);
            // print it
            server_name_to_proxies.insert(name.to_owned(), proxies);
            print_server(&name, &server_name_to_proxies);
            if open {
                open_server(&name);
            }
            return;
        }
        _ => (),
    }

    if !server_names.is_empty() {
        // print the server_names
        for server_name in server_names {
            print_server(&server_name, &server_name_to_proxies);
        }
    } else {
        println!("No local server found.");
    }
}
