use crate::Error;
use cron::Schedule;
use futures::{future::BoxFuture, FutureExt, StreamExt};

use chrono::Utc;
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference, Time};
use kube::api::{ListParams, Patch, PatchParams, PostParams};
use kube::{Api, Client, CustomResource, Resource};
use kube_runtime::{
    controller::{Context, ReconcilerAction},
    Controller,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
struct JobTemplate {
    spec: JobSpec,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[schemars(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
#[kube(
    kind = "KrustJob",
    group = "kitagry.github.io",
    version = "v1alpha1",
    namespaced
)]
#[kube(status = "KrustJobStatus")]
pub struct KrustJobSpec {
    /// The schedule in Cron format, see https://en.wikipedia.org/wiki/Cron.
    #[schemars(length(min = 0))]
    schedule: String,

    /// Specifies the job that will be created when executing a CronJob.
    job_template: JobTemplate,
}

/// CronJobStatus defines the observed state of CronJob
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct KrustJobStatus {
    /// Information when was the last time the job was successfully scheduled.
    last_schedule_time: Option<Time>,
}

#[derive(Clone)]
pub struct Manager {}

#[derive(Clone)]
struct Data {
    client: Client,
}

async fn reconcile(cj: KrustJob, ctx: Context<Data>) -> Result<ReconcilerAction, Error> {
    let now = Time(Utc::now());
    let result = reconcile_job(&cj, ctx.get_ref().client.clone(), now.clone()).await?;

    let default_name = "".to_string();
    let pp = PatchParams::default();
    let data = serde_json::json!({
        "status": {
            "last_schedule_time": now.clone()
        }
    });
    let krustjobs = Api::<KrustJob>::namespaced(
        ctx.get_ref().client.clone(),
        &cj.metadata.namespace.as_ref().unwrap(),
    );
    krustjobs
        .patch_status(
            &cj.metadata.name.as_ref().unwrap_or_else(|| &default_name),
            &pp,
            &Patch::Merge(&data),
        )
        .await?;
    Ok(result)
}

async fn reconcile_job(
    cj: &KrustJob,
    client: kube::Client,
    now: Time,
) -> Result<ReconcilerAction, Error> {
    // when schedule comes, create a job.
    // and update status
    let schedule = Schedule::from_str(&cj.spec.schedule)?;
    let mut status = cj.status.clone().unwrap_or(KrustJobStatus::default());
    let last_schedule_time = status.last_schedule_time.as_ref().unwrap_or(&now);
    let mut it = schedule.after(&last_schedule_time.0);
    let next_time = it.next().unwrap();

    if next_time > now.0 {
        return Ok(ReconcilerAction {
            requeue_after: Some(convert_duration(next_time - now.0)?),
        });
    }

    let job = construct_job_for_cronjob(&cj, Time(next_time))?;
    let pp = PostParams::default();
    let jobs = Api::<Job>::namespaced(client, &cj.metadata.namespace.as_ref().unwrap());
    jobs.create(&pp, &job).await?;

    status.last_schedule_time = Some(now.clone());
    let next_time = it.next().unwrap();

    Ok(ReconcilerAction {
        requeue_after: Some(convert_duration(next_time - now.0)?),
    })
}

fn convert_duration(d: chrono::Duration) -> Result<std::time::Duration, Error> {
    d.to_std()
        .or_else(|e| Err(Error::ScheduleError(format!("schedule error: {}", e))))
}

fn construct_job_for_cronjob(cj: &KrustJob, scheduled_time: Time) -> Result<Job, Error> {
    let name = format!(
        "{}-{}",
        cj.metadata.name.as_ref().unwrap(),
        scheduled_time.0.timestamp()
    );

    let mut annotations = cj.metadata.annotations.clone().unwrap_or(BTreeMap::new());
    annotations.insert(
        "batch.tutorial.kubebuilder.io/scheduled-at".to_string(),
        scheduled_time.0.to_rfc3339(),
    );

    let owner_references = object_to_owner_reference::<KrustJob>(&cj.metadata)?;

    Ok(Job {
        metadata: ObjectMeta {
            namespace: cj.metadata.namespace.clone(),
            name: Some(name),
            labels: cj.metadata.labels.clone(),
            annotations: Some(annotations),
            owner_references: Some(vec![owner_references]),
            ..Default::default()
        },
        spec: Some(cj.spec.job_template.spec.clone()),
        ..Default::default()
    })
}

fn object_to_owner_reference<K: Resource<DynamicType = ()>>(
    meta: &ObjectMeta,
) -> Result<OwnerReference, Error> {
    Ok(OwnerReference {
        api_version: K::api_version(&()).to_string(),
        kind: K::kind(&()).to_string(),
        name: meta
            .name
            .as_ref()
            .ok_or(Error::MissingObjectKey {
                name: ".metadata.name",
            })?
            .to_string(),
        uid: meta
            .uid
            .as_ref()
            .ok_or(Error::MissingObjectKey {
                name: ".metadata.uid",
            })?
            .to_string(),
        ..OwnerReference::default()
    })
}

fn error_policy(error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    println!("reconcile failed: {:?}", error);
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(360)),
    }
}

impl Manager {
    pub async fn new() -> (Self, BoxFuture<'static, ()>) {
        let client = Client::try_default().await.expect("create client");
        let context = Context::new(Data {
            client: client.clone(),
        });

        let cronjobs = Api::<KrustJob>::all(client);
        let _r = cronjobs.list(&ListParams::default().limit(1)).await.expect(
            "is the crd installed? please run: cargo run --bin crdgen | kubectl apply -f -",
        );

        // All good. Start controller and return its future.
        let drainer = Controller::new(cronjobs, ListParams::default())
            .run(reconcile, error_policy, context)
            .filter_map(|x| async move { std::result::Result::ok(x) })
            .for_each(|_| futures::future::ready(()))
            .boxed();

        (Self {}, drainer)
    }
}
