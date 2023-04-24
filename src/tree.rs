use cpython::{NoArgs, ObjectProtocol, PyDict, Python};
use std::fs::{File, OpenOptions, read_to_string};
use std::io::{BufRead, BufReader, Write, Read};
use std::path::Path;


// Clone within node
pub fn add_node_to_level(id: &str, val: i32) -> Result<cpython::PyObject, cpython::PyErr> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let npar = get_parent(id).unwrap();
    let npar_dict = PyDict::new(py);
    npar_dict.set_item(py, "npar", npar).unwrap();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(npar)+'x')", Some(&npar_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();

    // Parent as kwarg
    let parent = PyDict::new(py) ;
    parent.set_item(py, "parent", par).unwrap();

    // Node value
    let node = anytree.call(py, "Node", (val,), Some(&parent));
    node
}

// Add child to node
pub fn add_node_to_parent(id: &str, val: i32) -> Result<cpython::PyObject, cpython::PyErr> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let id_dict = PyDict::new(py);
    id_dict.set_item(py, "id", id).unwrap();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(id)+'x')", Some(&id_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();

    // Parent as kwarg
    let parent = PyDict::new(py) ;
    parent.set_item(py, "parent", par).unwrap();

    // Node value
    let node = anytree.call(py, "Node", (val,), Some(&parent));
    node
}

// Add to root tree
pub fn append_base_tree(val: i32) -> Result<cpython::PyObject, cpython::PyErr> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Parent as kwarg
    let parent = PyDict::new(py) ;
    parent.set_item(py, "parent", tree.getattr(py, "root").unwrap()).unwrap();

    let node = anytree.call(py, "Node", (val,), Some(&parent));
    node
}

// Import fstree file
fn fstree() -> Result<cpython::PyObject, cpython::PyErr> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    // Import DictImporter and call import_ function
    let importer = py.import("anytree.importer").unwrap();
    let dict_importer = importer.get(py, "DictImporter").unwrap();
    let importer_instance = dict_importer.call(py, NoArgs, None).unwrap();

    // Import tree file
    let tree_file = import_tree_file("/.snapshots/ash/fstree").unwrap();

    // Call import_ function with tree_file argument
    let fstree = importer_instance.call_method(py, "import_", (tree_file,), None);
    fstree
}

// Get current snapshot
pub fn get_current_snapshot() -> String {
    let csnapshot = read_to_string("/usr/share/ash/snap").unwrap();
    csnapshot.trim_end().to_string()
}

// Get parent
pub fn get_parent(id: &str) -> Result<cpython::PyObject, cpython::PyErr> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let id_dict = PyDict::new(py);
    id_dict.set_item(py, "id", id).unwrap();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(id)+'x')", Some(&id_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();
    par.getattr(py, "parent").unwrap().getattr(py, "name")
}

// Import filesystem tree file
fn import_tree_file(treename: &str) -> Result<cpython::PyObject, cpython::PyErr> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    // Import ast python module
    let ast = py.import("ast").unwrap();

    // Read first line in tree file
    let treefile = File::open(treename).unwrap();
    let buf_read = BufReader::new(treefile);
    let mut read = buf_read.lines();
    let treefile_readline = read.next().unwrap().unwrap();

    // Use literal_eval from ast
    let tree_file = ast.get(py, "literal_eval").unwrap().call(py, (treefile_readline,), None);
    tree_file
}

// Print out tree with descriptions //REVIEW
/*pub fn print_tree() {
    let tree =fstree().unwrap();
    let snapshot = get_current_snapshot();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let id_dict = PyDict::new(py);
    id_dict.set_item(py, "id", &snapshot).unwrap();

    // From anytree import AsciiStyle, RenderTree
    let anytree =  py.import("anytree").unwrap();
    let asciistyle = anytree.call(py, "AsciiStyle", NoArgs, None).unwrap();
    let style = PyDict::new(py);
    style.set_item(py, "style", asciistyle).unwrap();
    let rendertree = anytree.call(py, "RenderTree", (&tree,), Some(&style)).unwrap().iter(py).unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(id)+'x')", Some(&id_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();

    // Parent as kwarg
    let parent = PyDict::new(py) ;
    parent.set_item(py, "parent", par).unwrap();

    // Node value
    let val: i32 = snapshot.parse().unwrap();
    let node = anytree.call(py, "Node", (val,), Some(&parent)).unwrap();
    let node_name = node.getattr(py, "name").unwrap().to_string();

    for (pre, fill, node) in rendertree {
        if Path::new(&format!("/.snapshots/ash/snapshots/{}-desc", node_name)).is_file() {
            let descfile = File::open(format!("/.snapshots/ash/snapshots/{}-desc", node_name)).unwrap();
            let mut desc = String::new();
            descfile.read_to_string(&mut desc).unwrap();
        } else {
            let desc = "";
        }
        if node_name == "0" {
            let desc = "base snapshot";
            if snapshot != node_name {
                println!("{}{}-{}", pre,node_name,desc);
            } else {
                println!("{}{}*-{}", pre,node_name,desc);//REVIEW
            }
        }
    }
}*/

// Return order to recurse tree
pub fn recurse_tree(cid: &str) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    for child in return_children(cid) {
        let par = get_parent(&child).unwrap().to_string();
        if child != cid {
            order.push(par);
            order.push(child);
        }
    }
    return order;
}

// Remove node from tree
pub fn remove_node(id: &str) -> Result<cpython::PyObject, cpython::PyErr> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let id_dict = PyDict::new(py);
    id_dict.set_item(py, "id", id).unwrap();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(id)+'x')", Some(&id_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let parent: Option<String> = None;
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();
    par.setattr(py, "parent", parent).unwrap();
    par.getattr(py, "parent")
}

// Return all children for node
pub fn return_children(id: &str) -> Vec<String> {
    let tree = fstree().unwrap();
    let gil = Python::acquire_gil();
    let py = gil.python();
    let mut children: Vec<String> = Vec::new();
    let id_dict = PyDict::new(py);
    id_dict.set_item(py, "id", id).unwrap();

    // Import anytree
    let anytree =  py.import("anytree").unwrap();

    // Filter as kwarg
    let filter = py.eval("lambda node: ('x'+str(node.name)+'x') in ('x'+str(id)+'x')", Some(&id_dict), None).unwrap();
    let filter_ = PyDict::new(py);
    filter_.set_item(py, "filter_", filter).unwrap();

    // Parent value
    let par = anytree.call(py, "find", (tree,), Some(&filter_)).unwrap();

    // Import PreOrderIter
    let preorderiter = anytree.call(py, "PreOrderIter", (par,), None).unwrap().iter(py).unwrap();

    for child in preorderiter {
        children.push(child.unwrap().getattr(py, "name").unwrap().to_string());
    }
    if let Some(index) = children.iter().position(|x| x == id) {
        children.remove(index);
    }
    children
}

//#   Sync tree helper function ### REVIEW might need to put it in distro-specific ashpk.py
//def sync_tree_helper(CHR, s_f, s_t):
    //os.system("mkdir -p /.snapshots/tmp-db/local/") ### REVIEW Still resembling Arch pacman folder structure!
    //os.system("rm -rf /.snapshots/tmp-db/local/*") ### REVIEW
    //pkg_list_to = pkg_list(CHR, s_t)
    //pkg_list_from = pkg_list("", s_f)
  //# Get packages to be inherited
    //pkg_list_from = [j for j in pkg_list_from if j not in pkg_list_to]
    //os.system(f"cp -r /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/. /.snapshots/tmp-db/local/") ### REVIEW
    //os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-{s_f}/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/{DEBUG}")
    //os.system(f"rm -rf /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/*") ### REVIEW
    //os.system(f"cp -r /.snapshots/tmp-db/local/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/") ### REVIEW
    //for entry in pkg_list_from:
        //os.system(f"bash -c 'cp -r /.snapshots/rootfs/snapshot-{s_f}/usr/share/ash/db/local/{entry}-[0-9]* /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/'") ### REVIEW
    //os.system("rm -rf /.snapshots/tmp-db/local/*") ### REVIEW (originally inside the loop, but I took it out)

// Save tree to file
pub fn write_tree() -> Result<(), std::io::Error> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    // Import DictExporter
    let exporter = py.import("anytree.exporter").unwrap();
    let dict_exporter = exporter.get(py, "DictExporter").unwrap();
    let exporter_instance = dict_exporter.call(py, NoArgs, None).unwrap();

    // Open & edit tree file
    let fstreepath = "/.snapshots/ash/fstree";
    let mut fsfile = OpenOptions::new().read(true)
                                       .write(true)
                                       .open(fstreepath)
                                       .unwrap();

    // Call export function with fstree argument
    let fstree = fstree().unwrap();
    let to_write = exporter_instance.call_method(py, "export", (fstree,), None);
    let write = fsfile.write_all(to_write.unwrap().to_string().as_bytes());
    write
}
