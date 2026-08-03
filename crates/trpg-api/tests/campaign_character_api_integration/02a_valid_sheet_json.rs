fn valid_sheet_json() -> String {
    serde_json::to_string(&Coc7CharacterSheet {
        name: "Evelyn Hart".to_owned(),
        age: 31,
        occupation: "Investigative journalist".to_owned(),
        era: "1920s".to_owned(),
        birthplace: "Brisbane".to_owned(),
        characteristics: Coc7Characteristics {
            strength: 50,
            dexterity: 60,
            power: 65,
            constitution: 55,
            size: 50,
            appearance: 55,
            intelligence: 70,
            education: 75,
            luck: 60,
        },
        skills: BTreeMap::from([
            ("Library Use".to_owned(), 70),
            ("Psychology".to_owned(), 55),
        ]),
        backstory_anchors: vec![
            "Protects confidential sources".to_owned(),
            "Distrusts official explanations".to_owned(),
        ],
    })
    .unwrap()
}
