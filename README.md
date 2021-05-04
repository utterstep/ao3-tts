# ao3-tts

Failsafe text-to-speech for AO3 HTML exports I've made for my wife :)

It generates TTSed MP3s for given AO3 HTML dump, along with ffmpeg input file to concatenate them.

## Prerequisites

* Google Cloud API account with [enabled Text-To-Speech API](https://cloud.google.com/text-to-speech/docs/quickstart-protocol?hl=en#before_you_begin)
* `ffmpeg` installed on your device with MP3 support

## Example environment content

_you can specify all vars in .env file, this crate uses dotenv internally_

```sh
# Google Cloud API Service Account Email
GAPI_SERVICE_ACCOUNT_EMAIL=some-account@some-project.iam.gserviceaccount.com
# GOOGLE_APPLICATION_CREDENTIALS from gcloud docs
GAPI_CREDS_FILE=/Users/user/gcp/some-project-31ad63427375.json
# HTML file exported from AO3
AO3_HTML_EXPORT_FILE=./all-the-young-dudes.html
```

## TTS Specifics

Currently, the TTS engine used is [Google Cloud Text-To-Speech API](https://cloud.google.com/text-to-speech) with [hardcoded parameters](./src/gcloud_api.rs#L50..L63).

TTSed text is dumped on drive automatically every 25 sentences, so that not much of paid for, generated text is lost in case of emergency.