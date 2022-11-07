--
-- PostgreSQL database dump
--

-- Dumped from database version 15.0
-- Dumped by pg_dump version 15.0

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: pool; Type: SCHEMA; Schema: -; Owner: postgres
--

CREATE SCHEMA pool;


ALTER SCHEMA pool OWNER TO postgres;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: block; Type: TABLE; Schema: pool; Owner: postgres
--

CREATE TABLE pool.block (
    id integer NOT NULL,
    height bigint NOT NULL,
    block_hash text NOT NULL,
    is_canonical boolean DEFAULT true,
    reward bigint NOT NULL,
    "timestamp" bigint DEFAULT EXTRACT(epoch FROM now()),
    paid boolean DEFAULT false NOT NULL,
    checked boolean DEFAULT false NOT NULL
);


ALTER TABLE pool.block OWNER TO postgres;

--
-- Name: block_id_seq; Type: SEQUENCE; Schema: pool; Owner: postgres
--

CREATE SEQUENCE pool.block_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE pool.block_id_seq OWNER TO postgres;

--
-- Name: block_id_seq; Type: SEQUENCE OWNED BY; Schema: pool; Owner: postgres
--

ALTER SEQUENCE pool.block_id_seq OWNED BY pool.block.id;


--
-- Name: share; Type: TABLE; Schema: pool; Owner: postgres
--

CREATE TABLE pool.share (
    id integer NOT NULL,
    block_id integer NOT NULL,
    miner text NOT NULL,
    share bigint NOT NULL
);


ALTER TABLE pool.share OWNER TO postgres;

--
-- Name: share_id_seq; Type: SEQUENCE; Schema: pool; Owner: postgres
--

CREATE SEQUENCE pool.share_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE pool.share_id_seq OWNER TO postgres;

--
-- Name: share_id_seq; Type: SEQUENCE OWNED BY; Schema: pool; Owner: postgres
--

ALTER SEQUENCE pool.share_id_seq OWNED BY pool.share.id;


--
-- Name: block id; Type: DEFAULT; Schema: pool; Owner: postgres
--

ALTER TABLE ONLY pool.block ALTER COLUMN id SET DEFAULT nextval('pool.block_id_seq'::regclass);


--
-- Name: share id; Type: DEFAULT; Schema: pool; Owner: postgres
--

ALTER TABLE ONLY pool.share ALTER COLUMN id SET DEFAULT nextval('pool.share_id_seq'::regclass);


--
-- Data for Name: block; Type: TABLE DATA; Schema: pool; Owner: postgres
--

COPY pool.block (id, height, block_hash, is_canonical, reward, "timestamp", paid, checked) FROM stdin;
\.


--
-- Data for Name: share; Type: TABLE DATA; Schema: pool; Owner: postgres
--

COPY pool.share (id, block_id, miner, share) FROM stdin;
\.


--
-- Name: block_id_seq; Type: SEQUENCE SET; Schema: pool; Owner: postgres
--

SELECT pg_catalog.setval('pool.block_id_seq', 129, true);


--
-- Name: share_id_seq; Type: SEQUENCE SET; Schema: pool; Owner: postgres
--

SELECT pg_catalog.setval('pool.share_id_seq', 1, false);


--
-- Name: block block_pk; Type: CONSTRAINT; Schema: pool; Owner: postgres
--

ALTER TABLE ONLY pool.block
    ADD CONSTRAINT block_pk PRIMARY KEY (id);


--
-- Name: share share_pk; Type: CONSTRAINT; Schema: pool; Owner: postgres
--

ALTER TABLE ONLY pool.share
    ADD CONSTRAINT share_pk PRIMARY KEY (id);


--
-- Name: block_block_hash_uindex; Type: INDEX; Schema: pool; Owner: postgres
--

CREATE UNIQUE INDEX block_block_hash_uindex ON pool.block USING btree (block_hash);


--
-- Name: block_checked_index; Type: INDEX; Schema: pool; Owner: postgres
--

CREATE INDEX block_checked_index ON pool.block USING btree (checked);


--
-- Name: block_height_index; Type: INDEX; Schema: pool; Owner: postgres
--

CREATE INDEX block_height_index ON pool.block USING btree (height);


--
-- Name: block_paid_index; Type: INDEX; Schema: pool; Owner: postgres
--

CREATE INDEX block_paid_index ON pool.block USING btree (paid);


--
-- Name: share share_block_id_fk; Type: FK CONSTRAINT; Schema: pool; Owner: postgres
--

ALTER TABLE ONLY pool.share
    ADD CONSTRAINT share_block_id_fk FOREIGN KEY (block_id) REFERENCES pool.block(id);


--
-- PostgreSQL database dump complete
--

