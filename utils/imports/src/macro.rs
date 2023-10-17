#[macro_export]
macro_rules! dynamic_import_table {
    (
        $(#[$struct_meta:meta])*
        $visibility:vis imports $name:ident {
            $(pub $var_name:ident: $var_type:ty = $var_init:expr,)*
        }
    ) => {
        $crate::paste! {
            #[allow(non_camel_case_types, non_snake_case, unused)]
            $(#[$struct_meta])*
            $visibility struct [<_ $name>] {
                $(pub $var_name: $var_type,)*
            }

            impl [<_ $name>] {
                pub fn resolve() -> $crate::ImportResult<Self> {
                    use $crate::DynamicImport;
                    use $crate::obfstr;
                    use anyhow::Context;

                    Ok(Self {
                        $(
                            $var_name: ($var_init).resolve()?,
                        )*
                    })
                }
            }

            $visibility static $name: $crate::DynamicImportTable<[<_ $name>]> = $crate::DynamicImportTable::new(&[<_ $name>]::resolve);
        }
    };
}

pub use dynamic_import_table;