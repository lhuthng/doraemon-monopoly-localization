import './app.css';
import Router from './Router.svelte';
import { mount } from 'svelte';

const app = mount(Router, { target: document.getElementById('app')! });

export default app;
