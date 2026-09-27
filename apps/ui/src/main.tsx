import "./styles/index.css";
import { bootstrap, renderLastResort } from "./app/bootstrap/bootstrap";

const container = document.getElementById("root");
if (!container) throw new Error("index.html has no #root element");
bootstrap(container).catch((error: unknown) => renderLastResort(container, error));
