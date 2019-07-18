extern crate clap;
extern crate reqwest;
extern crate roxmltree;
extern crate select;
extern crate scraper;
#[macro_use]
extern crate tantivy;

use std::path::Path;
use std::process::Command;
use clap::{App, Arg};
use reqwest::StatusCode;
use roxmltree::Document;
use tantivy::schema::Schema;
use tantivy::{Index, Score, DocAddress};
use tantivy::query::QueryParser;
use tantivy::collector::TopDocs;

const DEFAULT_INDEX_PATH: &str = "/tmp/gel";

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
    index_path: String,
}

fn get_url(text: String) -> Vec<Post> {
    let doc = Document::parse(&text).expect("xml parsing fail");
    doc.descendants()
        .filter(|node| { node.tag_name().name() == "post" })
        .map(|node| {
            let link = node.attribute("href").expect("attribute fail").to_string();
            let title = node.attribute("description").expect("attribute fail").to_string();
            Post{link, title}
        })
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

fn get_index(schema_path: &Path, schema: Schema) -> Index {
    let maybe_index = Index::open_in_dir(schema_path);
    if maybe_index.is_ok() {
        maybe_index.unwrap()
    } else {
        Index::create_in_dir(schema_path, schema).unwrap()
    }
}

fn scrape_posts(posts: Vec<Post>, schema_dir: String) {
    let mut schema_builder = Schema::builder();
    let title = schema_builder.add_text_field("title", tantivy::schema::TEXT | tantivy::schema::STORED);
    let body = schema_builder.add_text_field("body", tantivy::schema::TEXT);
    let schema = schema_builder.build();

    let schema_path = Path::new(&schema_dir);
    let index = get_index(schema_path, schema.clone());

    let mut index_writer = index.writer(100_000_000).expect("writer create fail");

    for post in posts {
        let post_title = post.title.clone();
        println!("Parsing {}", post.title);
        let text = get_text(post);
        let document = scraper::Html::parse_document(text.as_str());
        let selector = scraper::Selector::parse("p").expect("selector parse fail");
        let full_text = document.select(&selector).into_iter()
            .map(|paragraph| {
                paragraph.text().collect::<Vec<&str>>().join(" ")
            })
            .collect::<Vec<String>>()
            .join("\n");

        index_writer.add_document(doc!(
        title => post_title,
        body => full_text,
    ));
    }
    index_writer.commit().expect("commit fail");

    let reader = index.reader().expect("reader fail");
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    let query = query_parser.parse_query("rust").expect("query parse fail");

    let top_docs: Vec<(Score, DocAddress)> = searcher.search(&query, &TopDocs::with_limit(10))
        .expect("search fail");

    for (_score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address).expect("retrieve fail");
        println!("{}", schema.to_json(&retrieved_doc));
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
        .arg(Arg::with_name("index")
             .short("i")
             .long("index-path")
             .takes_value(true)
             .default_value(DEFAULT_INDEX_PATH))
        .get_matches();
    let pass_secret = matches.value_of("secret").expect("failed getting secret");
    let count: u64 = matches.value_of("count").expect("failed getting count").parse()
        .expect("failed parsing int");
    let index_path = matches.value_of("index").unwrap();

    let options = Options{secret: pass_secret.to_string(), count: count, index_path: index_path.to_string()};
    let index_path = options.index_path.clone();
    let posts = get_posts(options);
    scrape_posts(posts, index_path)
}
