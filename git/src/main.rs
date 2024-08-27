use git2::build::{CheckoutBuilder, CloneLocal, RepoBuilder};
use git2::{self, Cred, Time};
use git2::{
    Commit, Error, FetchOptions, ObjectType, RemoteCallbacks, Repository, RepositoryInitOptions,
    ResetType,
};
use rust_embed::RustEmbed;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread::Thread;
use std::time::Instant;
use std::{env, io, process, thread};
use xxhash_rust::const_xxh3::xxh3_64 as const_xxh3;

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
    let mut cb = git2::RemoteCallbacks::new();

    // Print out our transfer progress.
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        io::stdout().flush().unwrap();
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);
    // Always fetch all tags.
    // Perform a download and also update tips
    fo.download_tags(git2::AutotagOption::All);
    println!("Fetching {} for repo", remote.name().unwrap());
    remote.fetch(refs, Some(&mut fo), None)?;

    // If there are local objects (we got a thin pack), then tell the user
    // how many objects we saved from having to cross the network.
    let stats = remote.stats();
    if stats.local_objects() > 0 {
        println!(
            "\rReceived {}/{} objects in {} bytes (used {} local \
             objects)",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
            stats.local_objects()
        );
    } else {
        println!(
            "\rReceived {}/{} objects in {} bytes",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
    }

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let c = repo.reference_to_annotated_commit(&fetch_head)?;
    // println!("download-----------------------------------");
    // remote.download(refs, Some(&mut fo))?;

    // let stats = remote.stats();
    // if stats.local_objects() > 0 {
    //     println!(
    //         "\rReceived {}/{} objects in {} bytes (used {} local \
    //          objects)",
    //         stats.indexed_objects(),
    //         stats.total_objects(),
    //         stats.received_bytes(),
    //         stats.local_objects()
    //     );
    // } else {
    //     println!(
    //         "\rReceived {}/{} objects in {} bytes",
    //         stats.indexed_objects(),
    //         stats.total_objects(),
    //         stats.received_bytes()
    //     );
    // }
    Ok(c)
}

fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

fn normal_merge(
    repo: &Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let local_tree = repo.find_commit(local.id())?.tree()?;
    let remote_tree = repo.find_commit(remote.id())?.tree()?;
    let ancestor = repo
        .find_commit(repo.merge_base(local.id(), remote.id())?)?
        .tree()?;
    let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

    if idx.has_conflicts() {
        println!("Merge conflicts detected...");
        repo.checkout_index(Some(&mut idx), None)?;
        return Ok(());
    }
    let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    // now create the merge commit
    let msg = format!("Merge: {} into {}", remote.id(), local.id());
    let sig = repo.signature()?;
    let local_commit = repo.find_commit(local.id())?;
    let remote_commit = repo.find_commit(remote.id())?;
    // Do our merge commit and set current branch head to that commit.
    let _merge_commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;
    // Set working tree to match head.
    repo.checkout_head(None)?;
    Ok(())
}

fn do_merge<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    // 1. do a merge analysis
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    // 2. Do the appropriate merge
    if analysis.0.is_fast_forward() {
        println!("Doing a fast forward");
        // do a fast forward
        let refname = format!("refs/heads/{}", remote_branch);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                fast_forward(repo, &mut r, &fetch_commit)?;
            }
            Err(_) => {
                // The branch doesn't exist so just set the reference to the
                // commit directly. Usually this is because you are pulling
                // into an empty repository.
                repo.reference(
                    &refname,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
                )?;
                repo.set_head(&refname)?;
                repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        };
    } else if analysis.0.is_normal() {
        // do a normal merge
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        normal_merge(&repo, &head_commit, &fetch_commit)?;
    } else {
        println!("Nothing to do...");
    }
    Ok(())
}

struct Repo {
    url: String,
    path: String,
    branch: String,
}

impl Repo {
    fn reset(&self, path: &Path) {
        let repo = match Repository::open(path) {
            Ok(repo) => repo,
            Err(e) => panic!("Failed to open: {}", e),
        };
        repo.reset(
            &repo.revparse_single("HEAD").unwrap(),
            ResetType::Hard,
            None,
        )
        .unwrap();
    }

    fn clone(&self) {
        let mut rb = RepoBuilder::new();
        let mut fo = FetchOptions::new();
        let mut rc = RemoteCallbacks::new();
        println!("开始下载");
        // rc.transfer_progress(|p| {
        //     println!(
        //         "总对象数: {}, 增量对象: {}, 已进行哈希处理: {}, 已经行哈希处理增量: {}, 已下载对象: {}, 已注入本地对象: {}, 已接受包: {}",
        //         p.total_objects(),
        //         p.total_deltas(),
        //         p.indexed_objects(),
        //         p.indexed_deltas(),
        //         p.received_objects(),
        //         p.local_objects(),
        //         p.received_bytes()
        //     );
        //     true
        // });
        rc.credentials(|_url, username_from_url, _allowed_types| {
            println!("_url:{:#?}", _url);
            println!("username_from_url:{:#?}", username_from_url);
            println!("_allowed_types:{:#?}", _allowed_types);
            Cred::userpass_plaintext("13433001217", "qwe513521qq")
        });
        fo.remote_callbacks(rc);
        let rp = match rb
            .fetch_options(fo)
            // .clone_local(CloneLocal::Auto)
            .clone(&self.url, &self.path.as_ref())
        {
            Ok(repo) => repo,
            Err(e) => panic!("init 失败: {}", e),
        };
        // let repo = match Repository::clone(&self.url, &self.path) {
        //     Ok(repo) => repo,
        //     Err(e) => panic!("failed to init: {}", e),
        // };
    }

    fn find_last_commit<'repo>(&self, repo: &'repo Repository) -> Result<Commit<'repo>, Error> {
        let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
        match obj.into_commit() {
            Ok(c) => Ok(c),
            _ => Err(Error::from_str("commit error")),
        }
    }

    fn pull(&self, path: &Path) -> Result<(), Error> {
        let repo = Repository::open(path)?;
        let mut remote = repo.find_remote("origin")?;
        let fetch_commit = do_fetch(&repo, &[&self.branch], &mut remote)?;
        let _ = do_merge(&repo, &self.branch, fetch_commit);

        // let repo = Repository::open(path)?;

        // println!("is_worktree: {:#?}", repo.is_bare());

        // let mut f = FetchOptions::new();
        // let mut rc = RemoteCallbacks::new();
        // println!("开始下载");
        // rc.transfer_progress(|p| {
        //     println!(
        //         "总对象数: {}, 增量对象: {}, 已进行哈希处理: {}, 已经行哈希处理增量: {}, 已下载对象: {}, 已注入本地对象: {}, 已接受包: {}",
        //         p.total_objects(),
        //         p.total_deltas(),
        //         p.indexed_objects(),
        //         p.indexed_deltas(),
        //         p.received_objects(),
        //         p.local_objects(),
        //         p.received_bytes()
        //     );
        //     true
        // });
        // rc.update_tips(|s, o1, o2| {
        //     println!("s: {:#?}, o1: {:#?}, o2: {:#?}", s, o1, o2);
        //     true
        // });
        // f.remote_callbacks(rc);

        // repo.find_remote("origin")?
        //     .fetch(&[&self.branch], Some(&mut f), None)?;

        // let last_commit = self.find_last_commit(&repo)?;
        // let reference = repo.find_reference("FETCH_HEAD")?;
        // let fetched_commit = reference.peel_to_commit()?;
        // let index =
        //     repo.merge_commits(&last_commit, &fetched_commit, Some(&MergeOptions::new()))?;
        Ok(())
        // Ok(index);

        // let repo = Repository::open(path)?;
        //
        // repo.find_remote("origin")?
        //     .fetch(&[self.branch], None, None)?;
        //
        // let fetch_head = repo.find_reference("FETCH_HEAD")?;
        // let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        // let analysis = repo.merge_analysis(&[&fetch_commit])?;
        // if analysis.0.is_up_to_date() {
        //     Ok(())
        // } else if analysis.0.is_fast_forward() {
        //     let refname = format!("refs/heads/{}", self.branch);
        //     let mut reference = repo.find_reference(&refname)?;
        //     reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        //     repo.set_head(&refname)?;
        //     repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        // } else {
        //     Err(Error::from_str("Fast-forward only!"))
        // }
    }

    pub fn check(&self) {
        let repo_path = Path::new(&self.path);

        if !repo_path.exists() {
            self.clone();
            return;
        }

        if repo_path.exists() && repo_path.is_dir() {
            self.reset(repo_path);
            let idx = match self.pull(repo_path) {
                Ok(idx) => idx,
                Err(e) => panic!("Failed to pull: {}", e),
            };
        }
    }
}

#[derive(Debug, RustEmbed)]
#[folder = "dist/"]
struct Asset;

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    let line = http_request.get(0).unwrap();
    let arr: Vec<&str> = line.split(" ").collect();

    let mut path = arr[1];

    if "/" == path {
        path = "/index.html";
    }

    path = &path[1..];

    println!("path: {},{:#?}", path, arr[1]);

    let binding = Asset::get(path).unwrap();
    let html = std::str::from_utf8(binding.data.as_ref());

    let response = "HTTP/1.1 200 OK\r\n\r\n".to_owned() + &html.unwrap();

    stream.write(response.as_bytes()).unwrap();
}

// 调用 Windows api
// extern "C" {
//     fn Sleep(ms: u32);
// }

// 调用 dll 测试
// fn call_dll(){
//     unsafe {
//         println!("开始等待");
//         let mut start = Instant::now();
//         Sleep(500);
//         println!("等待结束, 耗时：{}", start.elapsed().as_millis());
//
//         start = Instant::now();
//         let lib = libloading::Library::new("E:\\Experimental\\callCDll\\libs\\Sadp.dll").unwrap();
//         println!("加载动态库, 耗时：{}", start.elapsed().as_millis());
//
//         start = Instant::now();
//         let func: libloading::Symbol<unsafe extern fn() -> u32> = lib.get(b"SADP_GetSadpVersion").unwrap();
//         let ver = func();
//         println!("执行, 耗时：{}", start.elapsed().as_millis());
//         println!("版本:{}", ver);
//     }
// }

// 运行js
// fn run_js() {
//
// }

fn main() {
    // call_dll();

    // run_js()

    // let listener = TcpListener::bind("127.0.0.1:3000").unwrap();
    // for stream in listener.incoming() {
    //     let stream = stream.unwrap();
    //
    //     thread::spawn(|| {
    //         handle_connection(stream);
    //     });
    // }
    // let start = Instant::now();
    // // let mut v = vec![];
    // for i in 2..3 {
    //     // let a = thread::spawn(move || {
    //     let name = format!("repo_{}", i);
    //     let start = Instant::now();
    //     let currencies = Repo {
    //         url: "https://gitee.com/caretop/caretop7_next.git".to_string(),
    //         // url: "https://gitee.com/openharmony/arkui_ace_engine.git".to_string(),
    //         path: name.clone(),
    //         branch: "master".to_string(),
    //     };
    //     currencies.check();
    //     let duration = start.elapsed();
    //     println!("[{}]: 耗时: {:?}", name, duration);
    //     // });
    //     // v.push(a);
    // }
    // // for e in v {
    // //     e.join().unwrap();
    // // }
    // let duration = start.elapsed();
    // println!("\n\n总耗时: {:?}", duration);

    // let start = Instant::now();
    // // let url = "https://gitee.com/openharmony/arkui_ace_engine.git";
    // let url = "https://gitee.com/y_project/RuoYi-App.git";
    // // let t1 = thread::spawn(move || {
    // //     clone(url, "clone_dir");
    // // });
    // // let t2 = thread::spawn(move || {
    // //     cmd(url, "cmd_dir");
    // // });
    // let t3 = thread::spawn(move || download(url, "download_dir"));
    // // t1.join().expect("t1 异常");
    // // t2.join().expect("t2 异常");
    // t3.join().expect("t3 异常");
    // let duration = start.elapsed();
    // println!("\n\n总耗时: {:?}", duration);
    let start = Instant::now();
    let mut s = DefaultHasher::new();
    let str = "hello word岁的法国看见帅哥受到了攻击防护谁有下次v白色乳液和, [] {}sdfg 世界各地饭后水果spigufhfsdvb _*R%#%#@$@?><~@";
    let mut r: u64 = 0;
    for _i in 1..10000 {
        // str.hash(&mut s);
        // r = s.finish();
        r = const_xxh3(str.as_bytes());
    }
    println!("测试:{}", r);
    println!("耗时:{:#?}", start.elapsed())
}

fn download(url: &str, path: &str) {
    let start = Instant::now();
    let mut rio = RepositoryInitOptions::new();
    rio.origin_url(url);
    let repo = git2::Repository::init_opts(path, &mut rio).expect("初始化异常");
    repo.remote_set_url("main", url)
        .expect("TODO: panic message");
    let mut cob = CheckoutBuilder::new();
    cob.recreate_missing(true);
    repo.checkout_head(Some(&mut cob)).expect("检出异常");

    // let mut r = repo.remote(path, url).expect("远端库异常");
    // println!("name:{:?}", r.name());
    // println!("url:{:?}", r.url());
    // let mut fo = FetchOptions::new();
    // let mut rc = RemoteCallbacks::new();
    // // rc.transfer_progress(|_| {
    // //     println!("进度");
    // //     true
    // // });
    // fo.remote_callbacks(rc);
    // r.fetch(&["master"], Some(&mut fo), None)
    //     .expect("fetch 出错");
    let duration = start.elapsed();
    println!("download 耗时: {:?}", duration);
}

fn clone(url: &str, path: &str) {
    let start = Instant::now();
    match git2::Repository::clone(url, path) {
        Ok(repo) => {
            println!("clone success");
            repo
        }
        Err(e) => panic!("failed to clone: {}", e),
    };
    let duration = start.elapsed();
    println!("clone 耗时: {:?}", duration);
}

fn cmd(url: &str, path: &str) {
    let start = Instant::now();
    let _output = process::Command::new("git") // 指定要运行的命令为 "cmd"
        .arg("clone") // 添加参数 "/C" 表示执行完后关闭 CMD 窗口
        .arg(url)
        .arg(path)
        .output() // 获取输出结果
        .expect("cmd clone 异常"); // 如果发生错误则 panic

    // println!(
    //     "cmd clone 结果:\n{}",
    //     String::from_utf8(output.stdout).unwrap()
    // ); // 打印标准输出内容
    let duration = start.elapsed();
    println!("cmd clone 耗时: {:?}", duration);
}
