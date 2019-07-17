extern crate clap;
extern crate reqwest;
extern crate roxmltree;
extern crate select;
extern crate scraper;

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

struct Options {
    secret: String,
    count: u64,
}

fn get_url(text: String) -> Vec<Post> {
    let doc = Document::parse(&text).expect("xml parsing fail");
    doc.descendants()
        .filter(|node| { node.tag_name().name() == "post" })
        .map(|node| {
            let link = node.attribute("href").expect("attribute fail").to_string();
            let title = node.attribute("description").expect("attribute fail").to_string();
            Post{link, title} })
        .collect::<Vec<Post>>()
}

fn get_posts(options: Options) -> Vec<Post> {
    let token = get_token(options.secret);
    let tokens: Vec<&str> = token.split(": ").collect();
    let auth = tokens[tokens.len() - 1].to_string();
    let url = format!("https://api.pinboard.in/v1/posts/recent?auth_token={auth}&count={count}",
                      auth=auth, count=options.count);
    let mut response = reqwest::get(url.as_str()).expect("failage");
    assert!(response.status() == StatusCode::OK);
    get_url(response.text().expect("response"))
}

fn get_text(post: Post) -> String {
    reqwest::get(post.link.as_str()).expect("crawl fail") .text() .expect("text fail")
}

fn scrape_post(post: Post) {
    let text = get_text(post);
    let document = scraper::Html::parse_document(text.as_str());
    let selector = scraper::Selector::parse("p").expect("selector parse fail");
    for paragraph in document.select(&selector) {
        let paragraph_text = paragraph.text().collect::<Vec<_>>().join(" ");
        println!("{}", paragraph_text);
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
        .arg(Arg::with_name("count")
            .short("c")
            .long("count")
            .help("number of recent posts")
            .default_value("1"))
        .get_matches();
    let pass_secret = matches.value_of("secret").expect("failed getting secret");
    let count: u64 = matches.value_of("count").expect("failed getting count").parse()
        .expect("failed parsing int");

    let posts = get_posts(Options{secret: pass_secret.to_string(), count});
    for post in posts {
        scrape_post(post)
    }
}
