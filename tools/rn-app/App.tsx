import React, {useState} from 'react';
import {Button, FlatList, SafeAreaView, StyleSheet, Switch, Text, View} from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import {QueryClient, QueryClientProvider, useQuery} from '@tanstack/react-query';
import {configureStore, createSlice} from '@reduxjs/toolkit';
import {Provider, useDispatch, useSelector} from 'react-redux';
import {create} from 'zustand';

// A small screen to try EmuLabZip's React Native tools on: nested components, props, a counter and a list, and the
// state tools: a Redux cart, a React Query fetch, a theme Context and a Zustand store.

// Redux: a cart.
const cart = createSlice({
  name: 'cart',
  initialState: {items: [] as {name: string; qty: number}[]},
  reducers: {
    add(state, action: {payload: string}) {
      const found = state.items.find(i => i.name === action.payload);
      if (found) {
        found.qty += 1;
      } else {
        state.items.push({name: action.payload, qty: 1});
      }
    },
    clear(state) {
      state.items = [];
    },
  },
});
const store = configureStore({reducer: {cart: cart.reducer}});

// Zustand: shown to EmuLabZip by registering it by name. Any store with getState() works the same way.
const useSettings = create<{units: string; toggle: () => void}>(set => ({
  units: 'metric',
  toggle: () => set(s => ({units: s.units === 'metric' ? 'imperial' : 'metric'})),
}));
(globalThis as any).__EMULABZIP_STORES__ = {settings: useSettings};

// Context: the theme.
const ThemeContext = React.createContext({mode: 'light'});
ThemeContext.displayName = 'ThemeContext';

const queryClient = new QueryClient();

// Counter and crash helpers, as before.
function Counter({label}: {label: string}) {
  const [count, setCount] = useState(0);
  return (
    <View style={styles.row} testID="counter">
      <Text>{label}: {count}</Text>
      <Button title="Tap" onPress={() => { setCount(count + 1); console.log('tapped', count + 1); }} />
    </View>
  );
}

// Nested calls, so the stack of a crash has a few frames of this file to read.
function price(item: {cost?: {amount: number}}) {
  return item.cost!.amount;
}

function checkout() {
  return price({});
}

function Fruit({name}: {name: string}) {
  const dispatch = useDispatch();
  return (
    <View style={styles.row}>
      <Text style={styles.item}>{name}</Text>
      <Button title="Add" onPress={() => dispatch(cart.actions.add(name))} />
    </View>
  );
}

function CartSummary() {
  const items = useSelector((s: ReturnType<typeof store.getState>) => s.cart.items);
  const dispatch = useDispatch();
  const total = items.reduce((n, i) => n + i.qty, 0);
  return (
    <View style={styles.row}>
      <Text>Cart: {total}</Text>
      <Button title="Clear" onPress={() => dispatch(cart.actions.clear())} />
    </View>
  );
}

// React Query: asks a plain HTTP page for the time, so no certificate is needed on the device.
function Clock() {
  const time = useQuery({
    queryKey: ['time'],
    queryFn: async () => (await (await fetch('http://time.akamai.com/')).text()).trim(),
  });
  return (
    <View style={styles.row}>
      <Text>Time: {time.data ?? time.status}</Text>
      <Button title="Refetch" onPress={() => time.refetch()} />
    </View>
  );
}

function Settings() {
  const units = useSettings(s => s.units);
  const toggle = useSettings(s => s.toggle);
  return (
    <View style={styles.row}>
      <Text>Units: {units}</Text>
      <Button title="Switch" onPress={toggle} />
    </View>
  );
}

export default function App() {
  const [enabled, setEnabled] = useState(true);
  const [dark, setDark] = useState(false);
  return (
    <Provider store={store}>
      <QueryClientProvider client={queryClient}>
        <ThemeContext.Provider value={{mode: dark ? 'dark' : 'light'}}>
          <SafeAreaView style={styles.container}>
            <Text style={styles.title}>React Native demo</Text>
            <Counter label="Taps" />
            <Button title="Crash" onPress={() => { checkout(); }} />
            <View style={styles.row}>
              <Text>Notifications</Text>
              <Switch value={enabled} onValueChange={setEnabled} />
            </View>
            <View style={styles.row}>
              <Text>Dark theme</Text>
              <Switch value={dark} onValueChange={setDark} />
            </View>
            <CartSummary />
            <Clock />
            <Settings />
            <Button
              title="Save to AsyncStorage"
              onPress={() => AsyncStorage.setItem('demo:prefs', JSON.stringify({enabled, dark, savedAt: Date.now()}))}
            />
            <FlatList
              data={['Apples', 'Bananas', 'Cherries', 'Dates']}
              keyExtractor={item => item}
              renderItem={({item}) => <Fruit name={item} />}
            />
          </SafeAreaView>
        </ThemeContext.Provider>
      </QueryClientProvider>
    </Provider>
  );
}

const styles = StyleSheet.create({
  container: {flex: 1, padding: 16},
  title: {fontSize: 24, fontWeight: '600', marginBottom: 12},
  row: {flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginVertical: 4},
  item: {paddingVertical: 8},
});
