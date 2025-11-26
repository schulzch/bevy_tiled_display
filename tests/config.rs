use bevy_tiled_display::*;

#[test]
fn config_load_vvand20_xml() {
    let xml = include_str!("../configs/vvand20.xml");
    let td: TiledDisplay = quick_xml::de::from_str(xml).expect("Failed to parse xml");

    // Basic sanity checks
    assert_eq!(td.name, "VVand");
    assert_eq!(td.width, 10800);
    assert_eq!(td.height, 4096);

    // Expect keshiki01..keshiki20
    assert_eq!(td.machines.len(), 20);
    assert_eq!(td.machines.first().unwrap().identity, "keshiki01");
    assert_eq!(td.machines.last().unwrap().identity, "keshiki20");
}
