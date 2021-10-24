use kube::CustomResourceExt;
fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&krustjob::KrustJob::crd()).unwrap()
    )
}
