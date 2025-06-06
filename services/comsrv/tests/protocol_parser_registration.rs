use comsrv::core::protocols::{init_protocol_parsers, common::combase::get_global_parser_registry};

#[test]
fn test_protocol_parser_registration() {
    init_protocol_parsers();
    let registry = get_global_parser_registry();
    let protocols = registry.registered_protocols();
    assert!(protocols.contains(&"Modbus".to_string()));
    assert!(protocols.contains(&"CAN".to_string()));
    assert!(protocols.contains(&"IEC60870".to_string()));
}

