---
source: 7f222a4d726b
description: 在 React Native 开发版应用运行时读取它的实时状态，包括 Redux、React Query、React context 和自定义 store，并用 --watch 跟踪 Redux action 引起的变化。
---
# React Native 状态

**Frameworks → React Native → State** 和 `emulabzip rn state` 会读取 React Native 开发版应用的实时状态：Redux、React Query、React context，以及你自己的代码注册的 store。`--watch` 会跟踪 Redux action，并打印每个 action 改变了什么。

```bash
emulabzip rn state com.example.shop
emulabzip rn state com.example.shop --watch
emulabzip rn state com.example.shop --json
```

它需要 [React Native](frameworks.md) 本来就需要的条件：开发构建、正在运行的 Metro，以及同一时间只有一个调试器。

## 读取的内容

| 来源 | 如何找到 |
|--------|-----------------|
| **Redux** | 一个 `Provider`（react-redux），其 `store` 属性带有 `getState`、`dispatch` 和 `subscribe`；状态即 `getState()` |
| **React Query**（TanStack Query） | 带有 `client` 的 provider：其缓存中的每个查询，包括 key、状态、获取状态、更新时间、错误和数据 |
| **React context** | 屏幕上的 context provider，按其 `displayName`（或外层组件）和 `value` 列出。Redux 和 React Query 自己的 context 不会重复列出 |
| **命名 store** | 你在 `globalThis.__EMULABZIP_STORES__` 中注册的任何内容 |

### Zustand、MobX、Jotai 等

按名称注册 store；EmuLabZip 会调用 `getState()`（或 `get()`），或直接读取该对象：

```ts
import {create} from 'zustand';
const useSettings = create(() => ({units: 'metric'}));
globalThis.__EMULABZIP_STORES__ = {settings: useSettings};
```

应用中不会安装任何东西，如果从不读取该名称，也不会发生任何事。在发布构建中，只有这种注册方式能用：React 在那里不保留 DevTools hook，因此找不到 Redux、React Query 和 context。

## 值如何显示

页面脚本会把状态转成 JSON，并说明它省略了什么。循环引用标记为 `{"$":"circular"}`，函数标记为 `{"$":"function"}`，Date、Map、Set、错误、bigint 和类型化数组则标记为各自的类型。它最多深入 8 层，每个对象最多 200 个键或项，每个字符串最多 2,000 个字符，总计约 1 MB，**cut** 列表会指出在哪里被截断。

## 跟踪 action

`--watch`（以及 **Watch changes**）会从此刻起包装每个 Redux store 的 `dispatch`，并保留最近 200 个 action 及其类型和负载。每秒它会打印新的 action，以及状态在路径层级上的差异：

```
action cart/add
  redux/cart/items/0/qty: 1 → 2
```

停止时，`dispatch` 会被还原，日志也会从页面中移除。这是本阶段中唯一会改动正在运行的应用的操作，而且只在你观察期间生效。在你开始之前就拿走了 `dispatch` 的组件会继续使用旧的那个，因此它的 action 要等它再次渲染后才会出现在日志中。

## 限制

- 不读取 Flutter 状态（Provider、Riverpod、Bloc）。
- Redux 通过 react-redux 的 `Provider` 找到；不经过它而使用的 store，注册后也能找到。
- 值是快照：非常大的状态会被截断，而 getter 有副作用的 store，其 `getState()` 会运行这些副作用。
