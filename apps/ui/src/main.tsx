import "./styles/index.css";
import { bootstrap } from "./app/bootstrap/bootstrap";

const container = document.getElementById("root");
if (!container) throw new Error("index.html has no #root element");
void bootstrap(container);
