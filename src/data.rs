use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Axes {
    pub color: Vec<String>,
    pub layout: Vec<String>,
    pub r#type: Vec<String>,
}

#[derive(Deserialize)]
pub struct Text {
    pub voice: Vec<String>,
    pub entry: Vec<String>,
    pub register: Vec<String>,
}

#[derive(Deserialize)]
pub struct Idea {
    pub verb: Vec<String>,
    pub constraint: Vec<String>,
    pub scale: Vec<String>,
    pub pressure: Vec<String>,
}

#[derive(Deserialize)]
pub struct Data {
    pub methods: BTreeMap<String, Vec<String>>,
    pub visual: Axes,
    pub text: Text,
    pub idea: Idea,
    pub anchor: Vec<String>,
    pub no_anchor: Vec<String>,
    pub setting: Vec<String>,
    pub region: Vec<String>,
    pub fixed_color: Vec<String>,
    pub clash: Vec<[BTreeMap<String, String>; 2]>,
    pub hand_methods: Vec<String>,
    pub machine_methods: Vec<String>,
    pub hand_types: Vec<String>,
    pub machine_types: Vec<String>,
}

pub fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/seed.json")).expect("embedded seed data"))
}
