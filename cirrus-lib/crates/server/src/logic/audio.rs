use std::{
    fs::File,
    path::{Path, PathBuf}, collections::{HashMap, HashSet, hash_map::DefaultHasher}, hash::{Hash, Hasher},
};

use aiff::reader::AiffReader;
use bson::oid::ObjectId;
use chrono::{DateTime, NaiveDateTime, Utc, TimeZone};
use cirrus_grpc::api::{
    AudioDataRes, AudioMetaRes
};
// use futures::{TryStreamExt};
use mongodb::{bson::{Document, doc}, options::FindOptions, results::DeleteResult};
use tokio::sync::{Mutex, MutexGuard};
use walkdir::{DirEntry, WalkDir};

use crate::{
    util, 
    model::{self, document}
};

pub struct AudioFile {}

impl AudioFile {
    pub fn read_meta(
        filepath: &str
    ) -> Result<AudioMetaRes, String> {
        // let file = File::open(filepath)?;
        let file = match File::open(filepath) {
            Ok(file) => file,
            Err(err) => return Err(String::from("failed to load file")),
        };

        let mut reader = AiffReader::new(file);
        // reader.read().unwrap();
        match reader.read() {
            Ok(_) => (),
            Err(err) => match err {
                aiff::chunks::ChunkError::InvalidID(id) => return Err(String::from("invalid id")),
                aiff::chunks::ChunkError::InvalidFormType(id) => return Err(String::from("invalid form type")),
                aiff::chunks::ChunkError::InvalidID3Version(ver) => return Err(String::from("invalid id3 version")),
                aiff::chunks::ChunkError::InvalidSize(exp, actual) => return Err(format!("invalid size, expected: {}, actual: {}", exp, actual)),
                aiff::chunks::ChunkError::InvalidData(msg) => return Err(msg.to_string()),
            },
        }

        let common = reader.form().as_ref().unwrap().common().as_ref().unwrap();
        let sound = reader.form().as_ref().unwrap().sound().as_ref().unwrap();

        Ok(AudioMetaRes {
            bit_rate: common.bit_rate as u32,
            block_size: sound.block_size,
            channels: sound.block_size,
            offset: sound.offset,
            sample_frames: common.num_sample_frames,
            sample_rate: common.sample_rate as u32,
            size: sound.size as u32,
        })
    }

    pub fn read_data(
        filepath: &str, 
        byte_start: usize, 
        byte_end: usize
    ) -> Result<AudioDataRes, String> {
        // let file = File::open(filepath)?;
        let file = match File::open(filepath) {
            Ok(file) => file,
            Err(err) => return Err(String::from("failed to load file")),
        };
    
        let mut reader = AiffReader::new(file);
        // reader.read().unwrap();
        match reader.read() {
            Ok(_) => (),
            Err(err) => match err {
                aiff::chunks::ChunkError::InvalidID(id) => return Err(String::from("invalid id")),
                aiff::chunks::ChunkError::InvalidFormType(id) => return Err(String::from("invalid form type")),
                aiff::chunks::ChunkError::InvalidID3Version(ver) => return Err(String::from("invalid id3 version")),
                aiff::chunks::ChunkError::InvalidSize(exp, actual) => return Err(format!("invalid size, expected: {}, actual: {}", exp, actual)),
                aiff::chunks::ChunkError::InvalidData(msg) => return Err(msg.to_string()),
            },
        }
    
        let reader_form_ref = reader.form().as_ref().unwrap();
        let data = reader_form_ref.sound().as_ref().unwrap();
        let mut audio_data_part = Vec::<u8>::new();
        audio_data_part.extend_from_slice(&data.sound_data[4*byte_start..4*byte_end]);
    
        Ok(AudioDataRes {
            content: audio_data_part
        })
    }
}

pub struct AudioLibrary {}

impl AudioLibrary {
    // * path not exist -> return not found
    // * path is added already -> return added already 
    pub async fn add_audio_library(
        mongodb_client: mongodb::Client,
        library_root: &Path
    ) -> Result<String, String> {
        if !library_root.exists() {
            return Err(String::from("not exists"))
        }

        // let mut library_root_string = library_root.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/");
        // library_root_string = library_root_string.replace("/", ",");
        // library_root_string = format!(",{},", library_root_string);

        // if let Some(res) = model::AudioLibrary::get_by_path(mongodb_client.clone(), library_root).await.unwrap() {
        //     return Err(format!("path '{:?}' already exists", res))
        // }

        if model::AudioLibraryRoot::check_exists_by_path(mongodb_client.clone(), library_root).await {
            return Err(format!("path '{:?}' already exists", library_root))
        }

        let audio_types = ["aiff"];

        // let mut libraries: HashMap<PathBuf, Vec<document::AudioFile>> = HashMap::new();
        let mut libraries = HashSet::new();
        let mut audio_file_docs: Vec<document::AudioFile> = Vec::new();

        let audio_file_entry_iter = WalkDir::new(library_root).into_iter()
            .filter_map(|item| item.ok())
            // .map(|item| item.unwrap())
            .filter(|item| 
                item.metadata().unwrap().is_file() && 
                item.path().extension() != None)
            .filter(|item| {
                let file_extension = item.path().extension().unwrap();
                audio_types.contains(&file_extension.to_str().unwrap())
            });

        for entry in audio_file_entry_iter.into_iter() {
            let parent_path = entry.path().parent().unwrap().to_owned();
            let parent_path_materialized = util::path::path_to_materialized(&parent_path);
            let modified_timestamp = util::path::get_timestamp(entry.path());
            // let path_modified_time = entry.metadata().unwrap().modified().unwrap();
            // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);
            
            let filename = entry.file_name().to_str().unwrap().to_owned();

            libraries.insert(parent_path);

            // let mut hasher = DefaultHasher::new();

            // let mut audio_file_doc = document::AudioFile {
            //     id: None,
            //     modified_timestamp,
            //     parent_path: parent_path_materialized,
            //     filename,
            //     audio_tag_refer: None,
            // };
            // audio_file_doc.hash(&mut hasher);
            // let audio_file_doc_hash = hasher.finish() as i64;
            // println!("afdh: {}", audio_file_doc_hash);
            // audio_file_doc.id = Some(audio_file_doc_hash); 
            // audio_file_docs.push(audio_file_doc);

            audio_file_docs.push(document::AudioFile {
                id: Some(mongodb::bson::oid::ObjectId::new()),
                modified_timestamp,
                parent_path: parent_path_materialized,
                filename,
                audio_tag_refer: None,
            });
        }

        // let libraries_docs = libraries.into_iter()
        //     .map(|(library_path, file_metadata_vec)| {
        //         // let mut path = parent_path.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/");
        //         // path = path.replace("/", ",");
        //         // 
        //         // path = format!(",{},", path);
                
        //         // let path_modified_time = parent_path.metadata().unwrap().modified().unwrap();
        //         // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);
        //         let id = library_path.to_str().unwrap().to_owned();
        //         let path = util::path::path_to_materialized(&library_path);
        //         let modified_timestamp = util::path::get_timestamp(&library_path);
                
        //         document::AudioLibrary {
        //             id,
        //             path: Some(path),
        //             modified_timestamp,
        //         }
        //     })
        //     .collect::<Vec<_>>();

        
        let libraries_docs = libraries.into_iter()
            .map(|library_path| {
                // let mut path = parent_path.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/");
                // path = path.replace("/", ",");
                // 
                // path = format!(",{},", path);
                
                // let path_modified_time = parent_path.metadata().unwrap().modified().unwrap();
                // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);
                let id = util::path::replace_with_common_separator(library_path.to_str().unwrap());
                let path = util::path::path_to_materialized(&library_path);
                let modified_timestamp = util::path::get_timestamp(&library_path);
                
                document::AudioLibrary {
                    id,
                    path: Some(path),
                    modified_timestamp,
                }
            })
            .collect::<Vec<_>>();

        // let mut path = library_root.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/");
        // path = path.replace("/", ",");
        // path = format!(",{},", path);

        // let path_modified_time = library_root.metadata().unwrap().modified().unwrap();
        // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);

        let audio_library_root_doc = document::AudioLibrary {
            id: library_root.to_str().unwrap().to_owned(),
            path: Some(util::path::path_to_materialized(library_root)),
            modified_timestamp: util::path::get_timestamp(library_root),
        };

        model::AudioLibraryRoot::create(mongodb_client.clone(), audio_library_root_doc).await.unwrap();

        model::AudioLibrary::create_many(mongodb_client.clone(), libraries_docs).await.unwrap();

        // model::AudioFile::create_many(mongodb_client.clone(), audio_file_docs).await.unwrap();
        let create_many_res = model::AudioFile::create_many(mongodb_client.clone(), audio_file_docs).await;

        // match create_many_res {
        //     Ok(res) => println!("{:?}", res),
        //     Err(err) => println!("{:?}", err.kind),
        // }

        match create_many_res {
            Ok(res) => return Ok(format!("{:?}", res)),
            Err(err) => return Err(format!("{:?}", err)),
        }

        // model::AudioLibraryContents::create_many(mongodb_client.clone(), libraries_docs).await.unwrap();

        // let path_modified_time = path.metadata().unwrap().modified().unwrap();
        // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);

        // let path_doc = document::AudioLibrary {
        //     id: None,
        //     path: String::from(path_str),
        //     modified_timestamp: path_modified_time.timestamp(),
        // };

        // let create_res = model::AudioLibrary::create(mongodb_client.clone(), path_doc).await;

        // Ok(())
    }

    pub async fn remove_audio_library(
        mongodb_client: mongodb::Client,
        path: &Path
    ) -> Result<DeleteResult, String> {
        // let path_str = path.to_str().unwrap();

        // if let None = model::AudioLibrary::get_by_path(mongodb_client.clone(), path).await.unwrap() {
        //     return Err(format!("path '{}' is not registered", path.to_str().unwrap()))
        // }

        if model::AudioLibraryRoot::check_exists_by_path(mongodb_client.clone(), path).await {
            return Err(format!("path '{:?}' already exists", path))
        }

        let delete_libraries_res = model::AudioLibrary::delete_by_path(mongodb_client.clone(), path).await;

        let delete_library_root_res = model::AudioLibraryRoot::delete_by_path(mongodb_client.clone(), path).await;

        Ok(delete_library_root_res)
    }

    pub async fn analyze_audio_library(
        mongodb_client: mongodb::Client,
    ) -> Result<(), String> {
        let audio_libraries = model::AudioLibraryRoot::get_all(mongodb_client.clone()).await;

        for audio_library in audio_libraries.into_iter() {
            let audio_files = model::AudioFile::get_by_library_path(mongodb_client.clone(), Path::new(&audio_library.id), true).await.unwrap();

            for audio_file in audio_files.iter() {
                // let audio_file_path = Path::new(&util::path::materialized_to_path(&audio_file.parent_path)).join(&audio_file.filename);
                // let audio_tag = Self::create_audio_tag(&audio_file_path).unwrap();
                let audio_tag = Self::create_audio_tag(&audio_file);
                let audio_tag_id = audio_tag.id.clone();
                
                // let tag_create_res = model::AudioTag::create(mongodb_client.clone(), audio_tag).await.unwrap();
                match model::AudioTag::create(mongodb_client.clone(), audio_tag).await {
                    Ok(_) => (),
                    // Err(err) => println!("duplicated audio file {:?}, tag id {:?} exists", audio_file_path, audio_tag_id),
                    Err(err) => return Err(format!("{}", err)),
                }

                let update_res = model::AudioFile::set_audio_tag_refer(mongodb_client.clone(), &audio_file.id.unwrap(), &audio_tag_id.unwrap()).await.unwrap();
                println!("ur: {:?}", update_res);
            }
        }

        Ok(())
    }

    pub async fn refresh_audio_library(
        mongodb_client: mongodb::Client,
    ) -> Result<(), String> {
        let audio_library_roots = model::AudioLibraryRoot::get_all(mongodb_client.clone()).await;
        let audio_types = vec!["aiff"];

        for audio_library_root in audio_library_roots.into_iter() {
            let audio_libraries = model::AudioLibrary::get_by_path(mongodb_client.clone(), Path::new(&audio_library_root.id)).await.unwrap();
            // let local_library_root = std::fs::read_dir(Path::new(&audio_library_root.id)).unwrap();
            let local_library_root = WalkDir::new(audio_library_root.id);
            
            let local_library_root_directories: HashSet<_> = local_library_root.into_iter()
                .map(|item| item.unwrap())
                .filter(|item| 
                    item.metadata().unwrap().is_dir())
                .map(|item| util::path::replace_with_common_separator(item.path().to_str().unwrap()))
                .collect();

            // let local_library_root_directories: Vec<_> = local_library_root.into_iter()
            //     .filter_map(|item| 
            //         item.unwrap().metadata().unwrap().is_dir())
            //     // .map(|item| util::path::replace_with_common_separator(item.path().to_str().unwrap()))
            //     .collect();

            // let local_library_root_directories = local_library_root_directories.iter()
            //     .map(|item| )

            let audio_library_hashset: HashSet<_> = audio_libraries.iter()
                .map(|item| item.id.to_owned())
                // .filter(|item| item != &audio_library_root.id)
                .collect();

            // new directories on local library root
            let new_libraries: HashSet<_> = local_library_root_directories.difference(&audio_library_hashset).collect();
            // deleted directories on local library root
            let deleted_libraries: HashSet<_> = audio_library_hashset.difference(&local_library_root_directories).collect();

            println!("nl: {:?}, dl: {:?}", new_libraries, deleted_libraries);

            if !new_libraries.is_empty() {
                let mut audio_file_docs: Vec<document::AudioFile> = Vec::new();

                for new_library in new_libraries.iter() {
                    // let mut audio_file_docs: Vec<document::AudioFile> = Vec::new();

                    let audio_file_entry_iter = WalkDir::new(new_library).into_iter()
                        .filter_map(|item| item.ok())
                        // .map(|item| item.unwrap())
                        .filter(|item| 
                            item.metadata().unwrap().is_file() && 
                            item.path().extension() != None)
                        .filter(|item| {
                            let file_extension = item.path().extension().unwrap();
                            audio_types.contains(&file_extension.to_str().unwrap())
                        });
            
                    for entry in audio_file_entry_iter.into_iter() {
                        let parent_path = entry.path().parent().unwrap().to_owned();
                        let parent_path_materialized = util::path::path_to_materialized(&parent_path);
                        let modified_timestamp = util::path::get_timestamp(entry.path());
                        // let path_modified_time = entry.metadata().unwrap().modified().unwrap();
                        // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time);
                        
                        let filename = entry.file_name().to_str().unwrap().to_owned();
            
                        // libraries.insert(parent_path);

                        // let mut hasher = DefaultHasher::new();

                        // let mut audio_file_doc = document::AudioFile {
                        //     id: None,
                        //     modified_timestamp,
                        //     parent_path: parent_path_materialized,
                        //     filename,
                        //     audio_tag_refer: None,
                        // };
                        // audio_file_doc.hash(&mut hasher);
                        // let audio_file_doc_hash = hasher.finish() as i64;
                        // audio_file_doc.id = Some(audio_file_doc_hash.try_into().unwrap()); 
                        // audio_file_docs.push(audio_file_doc);
                        audio_file_docs.push(document::AudioFile {
                            id: Some(mongodb::bson::oid::ObjectId::new()),
                            modified_timestamp,
                            parent_path: parent_path_materialized,
                            filename,
                            audio_tag_refer: None,
                        });
                    }
                }

                let new_libraries_docs = new_libraries.into_iter()
                    .map(|library_path| {
                        let id = util::path::replace_with_common_separator(library_path.as_str());
                        let library_path = Path::new(library_path);
                        let path = util::path::path_to_materialized(&library_path);
                        let modified_timestamp = util::path::get_timestamp(&library_path);
                        
                        document::AudioLibrary {
                            id,
                            path: Some(path),
                            modified_timestamp,
                        }
                    })
                    .collect::<Vec<_>>();
    
                // let audio_library_root_doc = document::AudioLibrary {
                //     id: library_root.to_str().unwrap().to_owned(),
                //     path: Some(util::path::path_to_materialized(library_root)),
                //     modified_timestamp: util::path::get_timestamp(library_root),
                // };
                model::AudioLibrary::create_many(mongodb_client.clone(), new_libraries_docs).await.unwrap();

                model::AudioFile::create_many(mongodb_client.clone(), audio_file_docs).await.unwrap();
            }

            if !deleted_libraries.is_empty() {
                for deleted_library in deleted_libraries.into_iter() {
                    println!("sync delete audio library: {:?}", deleted_library);

                    let audio_files = model::AudioFile::get_by_library_path(mongodb_client.clone(), Path::new(&deleted_library), false).await.unwrap();
                    let delete_audio_tag_ids: Vec<_> = audio_files.iter()
                        .filter_map(|item| item.audio_tag_refer)
                        // .map(|item| item.audio_tag_refer.un)
                        .collect();
                    
                    let audio_tag_delete_res = model::AudioTag::delete_by_ids(mongodb_client.clone(), delete_audio_tag_ids).await.unwrap();

                    let audio_file_delete_res = model::AudioFile::delete_by_selfs(mongodb_client.clone(), &audio_files).await.unwrap();

                    let library_delete_res = model::AudioLibrary::delete_by_path(mongodb_client.clone(), Path::new(deleted_library)).await.unwrap();
                }
            }

        }

        // for audio_library in audio_libraries.iter() {
            // let audio_files = model::AudioFile::get_by_library_path(mongodb_client.clone(), Path::new(&audio_library.id), true).await;

            // for library_content in audio_files.iter() {
            //     // compare modified timestamp
            //     let library_content_path = util::path::materialized_to_path(&library_content.path);
            //     let library_content_path_local = Path::new(&library_content_path);

            //     let library_content_path_local_modified = util::path::get_timestamp(library_content_path_local);
                
            //     if library_content_path_local_modified != library_content.modified_timestamp {
            //         println!("updated: {:?}", library_content_path);
            //         Self::update_audio_library(
            //             mongodb_client.clone(),
            //             library_content_path_local,  
            //             library_content.contents.as_ref().unwrap(), 
            //             &audio_types).await;
            //     }
            // }



            // // let (path_slice_start, path_slice_end) = (1 as usize, audio_library.path.len() - 1);

            // // let path = &audio_library.path[path_slice_start..path_slice_end];
            // // let path = path.replace(",", "/");
            // let path = util::path::materialized_to_path(&audio_library.path);
            // let path = Path::new(&path);

            // // let path_modified_time = path.metadata().unwrap().modified().unwrap();
            // // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time).timestamp();
            // let path_modified_time = util::path::get_timestamp(path);

            // println!("{:?}, pmt: {}, almt: {}", path, path_modified_time, audio_library.modified_timestamp);

            // // Self::update_audio_library(mongodb_client.clone(), path, &audio_types).await;

            // // let libraries_content = model::AudioLibraryContents::get_by_materialized_path(mongodb_client.clone(), &audio_library.path).await.unwrap();
            // // println!("lc: {:?}", libraries_content);

            // if path_modified_time != audio_library.modified_timestamp {
            //     // path is modified
            //     println!("path {:?} modified", audio_library.path);

            //     let libraries_content = model::AudioLibraryContents::get_by_materialized_path(mongodb_client.clone(), &audio_library.path).await.unwrap();

            //     for library_content in libraries_content.iter() {
            //         // compare modified timestamp
            //         let library_content_path = util::path::materialized_to_path(&library_content.path);
            //         let library_content_path_local = Path::new(&library_content_path);

            //         let library_content_path_local_modified = util::path::get_timestamp(library_content_path_local);
                    
            //         if library_content_path_local_modified != library_content.modified_timestamp {
            //             println!("updated: {:?}", library_content_path);
            //         }
            //     }
                
            //     // let libraries_content = model::AudioLibraryContents::get_by_path(mongodb_client.clone(), &Path::new(&audio_library.path)).await.unwrap();
            //     // println!("lc: {:?}", libraries_content);

            //     // Self::update_audio_library(mongodb_client.clone(), path);
            //     // println!("path: {:?}, pmt: {:?}, alm: {:?}", audio_library.path, path_modified_time, audio_library.modified_timestamp);
            // }
        // }

        // collection; libraries - audio library root
        //             libraries-detail - actual contents (sub_dirs, audio_files)



        // filter updated path (by paths' modified datetime)

        Ok(())
    }

    fn create_audio_tag(
        // audio_file_path: &Path
        audio_file_doc: &document::AudioFile,
    ) -> document::AudioTag {
        let audio_file_path = Path::new(&util::path::materialized_to_path(&audio_file_doc.parent_path)).join(&audio_file_doc.filename);
        let audio_file = File::open(audio_file_path).unwrap();
        let mut aiff = AiffReader::new(audio_file);
        aiff.read().unwrap();

        let audio_metadata = if let Some(id3v2_tag) = aiff.id3v2_tag {
            let date_recorded = match id3v2_tag.date_recorded() {
                Some(datetime) => {
                    let month = datetime.month.unwrap_or_default();
                    let day = datetime.day.unwrap_or_default();
                    let hour = datetime.hour.unwrap_or_default();
                    let minute = datetime.minute.unwrap_or_default();
                    let second = datetime.second.unwrap_or_default();

                    Some(Utc.ymd(datetime.year, month.into(), day.into()).and_hms(hour.into(), minute.into(), second.into()))
                },
                None => None,
            };

            let date_released = match id3v2_tag.date_released() {
                Some(datetime) => {
                    let month = datetime.month.unwrap_or_default();
                    let day = datetime.day.unwrap_or_default();
                    let hour = datetime.hour.unwrap_or_default();
                    let minute = datetime.minute.unwrap_or_default();
                    let second = datetime.second.unwrap_or_default();

                    Some(Utc.ymd(datetime.year, month.into(), day.into()).and_hms(hour.into(), minute.into(), second.into()))
                },
                None => None,
            };

            println!("dr: {:?}, dr: {:?}", date_recorded, date_released);

            // let pictures = 

            let pictures: Vec<_> = id3v2_tag.pictures()
                .into_iter()
                .map(|item| document::AudioFileMetadataPicture {
                    description: item.description.clone(),
                    mime_type: item.mime_type.clone(),
                    picture_type: item.picture_type.to_string(),
                    data: item.data.to_owned(),
                })
                .collect();

            // for pic in id3v2_tag.pictures() {
            //     println!("pic description: {}, mime_type: {}, picture_type: {}", pic.description, pic.mime_type, pic.picture_type);
            // }

            let artist = match id3v2_tag.artist() {
                Some(item) => Some(item.to_owned()),
                None => None,
            };

            let album = match id3v2_tag.album() {
                Some(item) => Some(item.to_owned()),
                None => None,
            };

            let album_artist = match id3v2_tag.album_artist() {
                Some(item) => Some(item.to_owned()),
                None => None,
            };

            let genre = match id3v2_tag.genre() {
                Some(item) => Some(item.to_owned()),
                None => None,
            };

            let title = match id3v2_tag.title() {
                Some(item) => Some(item.to_owned()),
                None => None,
            };

            let mut audio_tag = document::AudioTag {
                id: Some(ObjectId::new()),
                property_hash: None,
                artist: artist,
                album: album,
                album_artist: album_artist,
                date_recorded,
                date_released,
                disc: id3v2_tag.disc(),
                duration: id3v2_tag.duration(),
                genre: genre,
                pictures: pictures,
                title: title,
                total_discs: id3v2_tag.total_discs(),
                total_tracks: id3v2_tag.total_tracks(),
                track: id3v2_tag.track(),
                year: id3v2_tag.year(),
            };

            audio_tag.property_hash = Some(util::hash::get_hashed_value(&audio_tag));

            return audio_tag

            // return Ok(document::AudioTag {
            //     id: Some(ObjectId::new()),
            //     artist: artist,
            //     album: album,
            //     album_artist: album_artist,
            //     date_recorded,
            //     date_released,
            //     disc: id3v2_tag.disc(),
            //     duration: id3v2_tag.duration(),
            //     genre: genre,
            //     pictures: pictures,
            //     title: title,
            //     total_discs: id3v2_tag.total_discs(),
            //     total_tracks: id3v2_tag.total_tracks(),
            //     track: id3v2_tag.track(),
            //     year: id3v2_tag.year(),
            // });

        } else {
            // println!("id3v2 tag is none");
            // return Err(format!("id3v2 tag is none"));
            return document::AudioTag {
                id: Some(ObjectId::new()),
                property_hash: None,
                title: Some(audio_file_doc.filename.clone().to_owned()),
                ..Default::default()
            };
            // None
        };

        // let audio_file = document::AudioFile {
        //     id: None,
        //     metadata: audio_metadata,
        //     modified_timestamp: path_modified_time,
        //     path: path_string,
        // };

    }

    // async fn analyze_audio_library(
    //     mongodb_client: mongodb::Client,
    //     path: &Path,
    // ) {

    // }

    async fn update_audio_library(
        mongodb_client: mongodb::Client,
        path: &Path,
        db_contents: &Vec<document::AudioFile>,
        audio_types: &Vec<&str>
    ) {
        println!("update path: {:?}", path);

        let paths = std::fs::read_dir(path).unwrap();

        let audio_file_entry_iter = paths.into_iter()
            .map(|item| item.unwrap())
            .filter(|item| 
                item.metadata().unwrap().is_file() && 
                item.path().extension() != None)
            .filter(|item| {
                let path = item.path();
                let file_extension = path.extension().unwrap();
                audio_types.contains(&file_extension.to_str().unwrap())
            });

        let previous_contents: HashMap<_, _> = db_contents.iter()
            .map(|value| (value.filename.as_str(), (value.id, value.modified_timestamp) ) )
            .collect();

        // for (filename, props) in previous_contents.iter() {
        //     println!("f: {}, props: {:?}", filename, props);
        // }

        for path in audio_file_entry_iter.into_iter() {
            // let path = path.unwrap();
            // println!("{:?}", path.file_name());
            if let Some((object_id, timestamp)) = previous_contents.get(&path.file_name().to_str().unwrap()) {
                println!("previous file: {:?}", path.path());
                if timestamp.to_owned() != util::path::get_timestamp(&path.path()) {
                    println!("updated file: {:?}", path.path());
                }
            } else {
                println!("new file: {:?}", path.file_name());
            }
        }
        // let mut current_path = "";
        // let walkdir = WalkDir::new(path);

        // // for entry in walkdir.into_iter().filter_map(|entry| Self::filter_file(entry.unwrap())) {
       
        //     // for entry in walkdir.into_iter() {
        // //     let entry = entry.unwrap();
        // //     let metadata = entry.metadata().unwrap();

        // //     if metadata.is_file() {
        // //         let filetype = metadata.file_type();
        // //         let path = entry.path();
        // //         let is_file = metadata.is_file();
    
        // //         println!("path: {:?}, filetype: {:?}, is_file: {:?}", path, path.extension().unwrap(), is_file);
        // //     }
        // // }

        // let audio_file_entry_iter = walkdir.into_iter()
        //     .map(|item| item.unwrap())
        //     .filter(|item| 
        //         item.metadata().unwrap().is_file() && 
        //         item.path().extension() != None)
        //     .filter(|item| {
        //         let file_extension = item.path().extension().unwrap();
        //         audio_types.contains(&file_extension.to_str().unwrap())
        //     });


        // for audio_file_entry in audio_file_entry_iter {
        //     println!("{:?}", audio_file_entry.path());

        //     let path_string = String::from(audio_file_entry.path().to_str().unwrap());
        //     let path_modified_time = util::path::get_timestamp(audio_file_entry.path());
        //     // let path_modified_time = audio_file_entry.path().metadata().unwrap().modified().unwrap();
        //     // let path_modified_time = DateTime::<chrono::Utc>::from(path_modified_time).timestamp();

        //     let audio_file = File::open(audio_file_entry.path()).unwrap();
        //     let mut aiff = AiffReader::new(audio_file);
        //     aiff.read().unwrap();

        //     let audio_metadata = if let Some(id3v2_tag) = aiff.id3v2_tag {
        //         let date_recorded = match id3v2_tag.date_recorded() {
        //             Some(datetime) => {
        //                 let month = datetime.month.unwrap_or_default();
        //                 let day = datetime.day.unwrap_or_default();
        //                 let hour = datetime.hour.unwrap_or_default();
        //                 let minute = datetime.minute.unwrap_or_default();
        //                 let second = datetime.second.unwrap_or_default();

        //                 Some(Utc.ymd(datetime.year, month.into(), day.into()).and_hms(hour.into(), minute.into(), second.into()))
        //             },
        //             None => None,
        //         };

        //         let date_released = match id3v2_tag.date_released() {
        //             Some(datetime) => {
        //                 let month = datetime.month.unwrap_or_default();
        //                 let day = datetime.day.unwrap_or_default();
        //                 let hour = datetime.hour.unwrap_or_default();
        //                 let minute = datetime.minute.unwrap_or_default();
        //                 let second = datetime.second.unwrap_or_default();

        //                 Some(Utc.ymd(datetime.year, month.into(), day.into()).and_hms(hour.into(), minute.into(), second.into()))
        //             },
        //             None => None,
        //         };

        //         println!("dr: {:?}, dr: {:?}", date_recorded, date_released);

        //         // let pictures = 

        //         let pictures: Vec<_> = id3v2_tag.pictures()
        //             .into_iter()
        //             .map(|item| document::AudioFileMetadataPicture {
        //                 description: item.description.clone(),
        //                 mime_type: item.mime_type.clone(),
        //                 picture_type: item.picture_type.to_string(),
        //                 data: item.data.to_owned(),
        //             })
        //             .collect();

        //         // for pic in id3v2_tag.pictures() {
        //         //     println!("pic description: {}, mime_type: {}, picture_type: {}", pic.description, pic.mime_type, pic.picture_type);
        //         // }

        //         let artist = match id3v2_tag.artist() {
        //             Some(item) => Some(item.to_owned()),
        //             None => None,
        //         };

        //         let album = match id3v2_tag.album() {
        //             Some(item) => Some(item.to_owned()),
        //             None => None,
        //         };

        //         let album_artist = match id3v2_tag.album_artist() {
        //             Some(item) => Some(item.to_owned()),
        //             None => None,
        //         };

        //         let genre = match id3v2_tag.genre() {
        //             Some(item) => Some(item.to_owned()),
        //             None => None,
        //         };

        //         let title = match id3v2_tag.title() {
        //             Some(item) => Some(item.to_owned()),
        //             None => None,
        //         };

        //         Some(document::AudioFileMetadata {
        //             id: None,
        //             artist: artist,
        //             album: album,
        //             album_artist: album_artist,
        //             date_recorded,
        //             date_released,
        //             disc: id3v2_tag.disc(),
        //             duration: id3v2_tag.duration(),
        //             genre: genre,
        //             pictures: pictures,
        //             title: title,
        //             total_discs: id3v2_tag.total_discs(),
        //             total_tracks: id3v2_tag.total_tracks(),
        //             track: id3v2_tag.track(),
        //             year: id3v2_tag.year(),
        //         })

        //     } else {
        //         println!("id3v2 tag is none");
        //         None
        //     };

        //     let audio_file = document::AudioFile {
        //         id: None,
        //         metadata: audio_metadata,
        //         modified_timestamp: path_modified_time,
        //         path: path_string,
        //     };

        //     model::Audio::create(mongodb_client.clone(), audio_file).await.unwrap();

        //     // println!("{:?}", aiff.id3v2_tag.);
        // }
    }

    // async fn create_audio_library(
    //     mongodb_client: mongodb::Client,
    // ) -> Result<(), String> {
    //     todo!()
    // }
}

enum AudioType {
    AIFF
}