import "./assets/fonts/Inter-Regular.ttf";
import "./assets/fonts/Inter-Medium.ttf";
import "./assets/fonts/Inter-SemiBold.ttf";
import { mount } from "nsh";
import { Shell } from "./Shell";

console.log(`rs-marker app-eval ${Date.now()}`);
mount(Shell);
