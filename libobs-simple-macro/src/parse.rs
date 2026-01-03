use syn::{
    parse::{Parse, ParseStream},
    Ident, LitStr, Result, Token, Type,
};

pub struct UpdaterInput {
    pub name: LitStr,
    pub updatable_type: Ident,
    pub underlying_ptr_type: Type,
}

impl Parse for UpdaterInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let n = input.parse()?;

        input.parse::<Token![,]>()?;
        let updatable_type = input.parse()?;

        input.parse::<Token![,]>()?;
        let underlying_ptr_type = input.parse()?;
        Ok(UpdaterInput {
            name: n,
            updatable_type,
            underlying_ptr_type,
        })
    }
}
