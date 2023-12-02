use anyhow::{Context, Error, Result};
use bincode;
use s3::{creds::Credentials, serde_types::Object, Bucket, BucketConfiguration, Region};
use tracing::{debug, info, trace};

use crate::encoding;

#[derive(Clone, Debug)]
pub struct Client {
    region: String,
    endpoint: String,
    access_key: String,
    secret_key: String,
}

impl Client {
    pub fn new(region: String, endpoint: String, access_key: String, secret_key: String) -> Self {
        debug!(
            endpoint = endpoint,
            region = region,
            "creating new s3 client"
        );
        return Client {
            region,
            endpoint,
            access_key,
            secret_key,
        };
    }

    pub async fn upload_chunk(
        &self,
        chunk: encoding::Chunk,
        bucket_name: &str,
        path: &str,
    ) -> Result<(), Error> {
        let bucket = self.bucket(bucket_name, true).await?;
        let data = bincode::serialize(&chunk).context("chunk serialization")?;
        let response_data = bucket.put_object(path, &data).await.context("put object")?;
        assert_eq!(response_data.status_code(), 200);
        info!(bucket = bucket_name, path = path, "uploaded block to s3");
        Ok(())
    }

    pub async fn download_chunk(
        &self,
        bucket_name: &str,
        path: &str,
    ) -> Result<encoding::Chunk, Error> {
        let bucket = self.bucket(bucket_name, false).await?;
        let response_data = bucket.get_object(path).await?;
        assert_eq!(response_data.status_code(), 200);
        let chunk: encoding::Chunk = bincode::deserialize(response_data.as_slice()).unwrap();
        debug!(
            bucket = bucket_name,
            path = path,
            "downloaded block from s3"
        );
        Ok(chunk)
    }

    pub async fn list_paths(&self, bucket_name: &str, path: &str) -> Result<Vec<String>, Error> {
        let bucket = self.bucket(bucket_name, false).await?;
        let prefix = path.to_string();
        let results = bucket.list(prefix, None).await?;

        let mut objects: Vec<Object> = Vec::new();
        for mut result in results {
            objects.append(&mut result.contents)
        }

        let paths: Vec<String> = objects.into_iter().map(|obj| obj.key).collect();
        trace!(bucket = bucket_name, path = path, "listed objects from s3");
        return Ok(paths);
    }

    async fn bucket(&self, bucket_name: &str, try_create: bool) -> Result<s3::Bucket, Error> {
        let region = Region::Custom {
            region: self.region.to_string(),
            endpoint: self.endpoint.to_string(),
        };
        let credentials = Credentials::new(
            Some(self.access_key.as_str()),
            Some(self.secret_key.as_str()),
            None,
            None,
            None,
        )?;

        let mut bucket =
            Bucket::new(bucket_name, region.clone(), credentials.clone())?.with_path_style();

        if try_create {
            if !bucket.exists().await? {
                bucket = Bucket::create_with_path_style(
                    bucket_name,
                    region,
                    credentials,
                    BucketConfiguration::default(),
                )
                .await
                .context("create bucket")?
                .bucket;
                debug!(bucket = bucket_name, "created bucket in s3");
            }
        }
        Ok(bucket)
    }
}
