## KrustJob

Minimal CronJob for Kubernetes.
This is very experimental project.

### How to use

#### Register CustomResource

```
cargo run --bin crdgen | kubectl create -f -
```

When you update CRD.

```
cargo run --bin crdgen | kubectl replace -f -
```

#### Start CustomController

```
cargo run --bin krustjob
```

#### Create new KrustJob

```
kubectl apply -f ./config/sample/krustjob.yaml
```
