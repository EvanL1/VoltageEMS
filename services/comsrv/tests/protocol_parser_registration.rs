use comsrv::core::protocols::{common::combase::get_global_parser_registry, init_protocol_parsers};

#[test]
fn test_protocol_parser_registration() {
    init_protocol_parsers();
    let registry = get_global_parser_registry();
    let registry = registry.read();
    let protocols = registry.registered_protocols();
    assert!(protocols.contains(&"Modbus".to_string()));
    assert!(protocols.contains(&"CAN".to_string()));
    assert!(protocols.contains(&"IEC60870".to_string()));
}
