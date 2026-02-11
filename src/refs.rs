fn test() {
    fn wants_str(a: Option<&str>) {
        match a {
            Some(a) => println!("{a}"),
            None => println!("nothing"),
        }
    }

    let opt_vec = Some(String::from("shit"));
    let ref_option_string = &opt_vec;
    let option_ref_string = ref_option_string.as_ref();
    let option_ref_str = ref_option_string.as_deref();

    wants_str(option_ref_str);
}
