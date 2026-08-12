// Procedural Macros //
// Trying to avoid proc macros, or at most make it a very low-cost compilation overhead
// I'll add more convenience methods later on that might cause higher compilation times, but
// for now ill be focusing on ones that are lower overhead and needed
// I will be avoiding syn entirely though

use proc_macro::{TokenStream, TokenTree};

// Simple case
#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    const MAX_SEARCH: u8 = 4;

    let mut tokens = input.into_iter();
    let mut index = 0;
    loop {
        index +=1;
        let current = tokens.next().unwrap();
        if let TokenTree::Ident(ident) = current  {
            if ident.to_string() == "struct" {
                break;
            }
        }
        if index == MAX_SEARCH {
            return TokenStream::new();
        }
    }

    match tokens.next().unwrap() {
        TokenTree::Ident(ident) => {
            format!(
                // ::tachyon3d_internal::___ gets the path to the trait
                "impl ::tachyon3d_internal::Resource for {} {{}}", ident.to_string()
            ).parse().unwrap()
        },
        _ => TokenStream::new(),
    }
}