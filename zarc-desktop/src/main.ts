import { mount } from 'svelte';
import App from './App.svelte';
import './style.css';

const target = document.getElementById('app');
if (!target) {
  throw new Error('无法找到挂载点 #app');
}

const app = mount(App, { target });

export default app;
