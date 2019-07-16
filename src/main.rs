extern crate clap;
extern crate reqwest;
extern crate roxmltree;

use std::process::Command;
use clap::{App, Arg};
use reqwest::StatusCode;
use roxmltree::Document;

fn get_token(secret_name: String) -> String {
    let output = Command::new("pass")
        .arg(secret_name)
        .output()
        .expect("fail pass");
    let lines = String::from_utf8(output.stdout).expect("failage");
    let v: Vec<&str> = lines.trim().split('\n').collect();
    v[v.len() - 1].to_string()
}

#[derive(Debug)]
struct Post {
    link: String,
    title: String,
}

fn get_url(text: String) -> Result<Post, Error> {
    let doc = Document::parse(&text).expect("xml parsing fail");
    for node in doc.descendants() {
        if node.tag_name().name() == "post" {
            let link = node.attribute("href").expect("attribute fail").to_string();
            let title = node.attribute("description").expect("attribute fail").to_string();
            let post = Post{
                link: link,
                title: title,
            };
            return Ok(post);
        }
    }
    Err(Error::new())
}

#[derive(Default, Debug)]
struct Error {
    message: String
}

impl Error {
    pub fn new() -> Error {
        Error::default()
    }
}

fn get_last(pass_secret: String) -> Result<Post, Error> {
    let token = get_token(pass_secret);
    let tokens: Vec<&str> = token.split(": ").collect();
    let auth = tokens[tokens.len() - 1].to_string();
    let url = format!("https://api.pinboard.in/v1/posts/recent?auth_token={}&count=1", auth);
    let mut response = reqwest::get(url.as_str()).expect("failage");
    match response.status() {
        StatusCode::OK => {
            Ok(get_url(response.text().expect("response")).expect("fail getting post"))
        },
        _ => {
            Err(Error::new())
        }
    }
}

fn main() {
    let matches = App::new("gel")
        .arg(Arg::with_name("secret")
             .short("s")
             .long("secret")
             .help("a pass secret containing API secret")
             .takes_value(true)
             .required(true))
        .get_matches();
    let pass_secret = matches.value_of("secret").expect("failed getting secret");

    match get_last(pass_secret.to_string()) {
        Ok(post) => println!("title: {title}, url: {url}", title=post.title, url=post.link),
        Err(_) => panic!("fail"),
    }
}
