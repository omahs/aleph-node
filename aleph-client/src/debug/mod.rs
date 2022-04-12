use crate::Connection;

mod aleph;
mod elections;
mod treasury;

fn pallet_prompt(name: &'static str) -> String {
    format!("-----------{}-----------", name)
}

fn entry_prompt(name: &'static str) -> String {
    format!("----{}", name)
}

fn element_prompt(el: String) -> String {
    format!("\t{}", el)
}

pub fn print_storages(connection: &Connection) {
    treasury::print_storage(connection);
    aleph::print_storage(connection);
    elections::print_storage(connection);
}
