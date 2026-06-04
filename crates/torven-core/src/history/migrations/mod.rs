use rusqlite_migration::{M, Migrations};

pub fn get_migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!("001_initial.sql"))])
}
