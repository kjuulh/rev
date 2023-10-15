use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(AppConfig)]
pub fn app_config_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as TokenStream2);

    let ast = syn::parse2::<DeriveInput>(input.clone()).unwrap();

    let struct_name = &ast.ident;

    let fields = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct
            .fields
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                let getter_name = format!("get_{}", field_name);
                let getter_name_ident = syn::Ident::new(&getter_name, field_name.span());
                let field_type = &f.ty;

                quote! {
                    pub fn #getter_name_ident(&self) -> #field_type {
                        self.#field_name.clone()
                    }
                }
            })
            .collect::<TokenStream2>(),
        _ => quote! {},
    };

    let expanded = quote! {
        impl #struct_name {
            #fields
        }

        impl TryFrom<inner_application_config::InnerApplicationConfig> for #struct_name {
            type Error = ::anyhow::Error;

            fn try_from(value: inner_application_config::InnerApplicationConfig) -> Result<Self, Self::Error> {
                Ok(Self {
                    committer: value.committer.ok_or(::anyhow::anyhow!("expected committer to be set"))?
                })
            }
        }

        pub mod inner_application_config {
            #[derive(Default)]
            pub struct InnerApplicationConfig {
                config_file: ::std::path::PathBuf,
                pub committer: Option<String>
            }

            impl ::clap::FromArgMatches for InnerApplicationConfig {
                fn from_arg_matches(matches: &::clap::ArgMatches) -> Result<Self, ::clap::error::Error> {
                    let mut matches = matches.clone();
                    Self::from_arg_matches_mut(&mut matches)
                }
                fn from_arg_matches_mut(matches: &mut ::clap::ArgMatches) -> Result<Self, ::clap::error::Error> {
                    use ::rev_config::{Env, ConfigFile};

                    let mut s = Self::default();

                    let config = if let Some(config_file) = matches.remove_one::<String>("config_file") {
                        ::std::path::PathBuf::from(config_file)
                    } else {
                        let project_home = std::env::var("REV_CONFIG_HOME")
                            .map(::std::path::PathBuf::from)
                            .unwrap_or_else(|_| {
                                let project = ::directories::ProjectDirs::from("io", "kjuulh", "rev")
                                    .expect("to be able to find XDG home variables");

                                project.config_dir().to_path_buf()
                            });


                        project_home.join("config.kdl")
                    };

                    if let Err(e) = s.set_from_config_file(&config) {
                        ::tracing::warn!("failed to read config from file: {e}");
                    }
                    if let Err(e) = s.set_from_env() {
                        ::tracing::warn!("failed to read config from env: {e}");
                    }

                    if let Some(committer) = matches.remove_one::<String>("committer") {
                        s.committer = Some(committer);
                    }

                    Ok(s)
                }
                fn update_from_arg_matches(&mut self, matches: &::clap::ArgMatches) -> Result<(), ::clap::error::Error> {
                    let mut matches = matches.clone();
                    self.update_from_arg_matches_mut(&mut matches)
                }
                fn update_from_arg_matches_mut(&mut self, matches: &mut ::clap::ArgMatches) -> Result<(), ::clap::error::Error> {
                    if let Some(committer) = matches.remove_one::<String>("committer") {
                        self.committer = Some(committer);
                    }
                    Ok(())
                }
            }

            impl ::clap::Args for InnerApplicationConfig {
                fn augment_args(cmd: ::clap::Command) -> ::clap::Command {
                    cmd
                    .arg(
                        ::clap::Arg::new("config_file")
                            .long("config-file")
                            .action(::clap::ArgAction::Set)
                    )
                    .arg(
                        ::clap::Arg::new("committer")
                            .long("committer")
                            .action(::clap::ArgAction::Set)
                    )
                }
                fn augment_args_for_update(cmd: ::clap::Command) -> ::clap::Command {
                    cmd
                    .arg(
                        ::clap::Arg::new("config_file")
                            .long("config-file")
                            .action(::clap::ArgAction::Set)
                    )
                    .arg(
                        ::clap::Arg::new("committer")
                            .long("committer")
                            .action(::clap::ArgAction::Set)
                    )
                }
            }

            impl ::rev_config::Env for InnerApplicationConfig {
                fn set_from_env(&mut self) -> Result<(), ::rev_config::EnvError> {
                    if let Ok(item) =  std::env::var("REV_COMMITTER") {
                        self.committer = Some(item);
                    }
                    Ok(())
                }

            }

            impl ::rev_config::ConfigFile for InnerApplicationConfig {
                fn set_from_config_file(&mut self, config_file: &::std::path::Path) -> Result<(), ::rev_config::ConfigFileError> {
                    pub use anyhow::Context;

                    let file_content = ::std::fs::read_to_string(config_file)
                        .context("failed to read confile file")
                        .map_err(|e| ::rev_config::ConfigFileError::ConfigFileError(e))?;

                    let doc: ::kdl::KdlDocument = file_content.parse()
                        .context("failed to parse kdl config file")
                        .map_err(|e| ::rev_config::ConfigFileError::ConfigFileError(e))?;

                    if let Some(config) = doc.get("config") {
                        if let Some(item) = config.get("committer").map(|i| i.value()) {
                            self.committer = item.as_string().map(|i| i.to_string());
                        }
                    }

                    Ok(())
                }
            }
        }
    };

    TokenStream::from(expanded)
}
